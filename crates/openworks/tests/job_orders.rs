mod memory;

use memory::{ok, send};
use openworks::Openworks;
use openworks_core::{Fail, Role, Row, Tx, Value};
use openworks_db::{AUDIT, EVENT, approvals, job_orders, jobs, members};

#[derive(Clone, Default)]
struct Store {
    orders: Vec<Row>,
    lines: Vec<Row>,
    asked: Vec<Row>,
    events: Vec<Value>,
    audits: usize,
}

fn text(value: &str) -> Value {
    Value::Text(value.to_owned())
}

fn job_row(id: &Value, person: &Value) -> Vec<Row> {
    let staffed = *person == Value::Null || *id == text("j1");
    if !staffed || ![text("j1"), text("j2")].contains(id) {
        return Vec::new();
    }
    let mut row = vec![
        id.clone(),
        text("p1"),
        text(&format!("J-2026-{}", if *id == text("j1") { 1 } else { 2 })),
    ];
    row.resize(13, Value::Null);
    vec![row]
}

impl Store {
    fn order_rows(&mut self, sql: &'static str, p: &[Value]) -> Vec<Row> {
        let of_job = |row: &&Row| row[0] == p[0] && row[2] == p[1];
        match sql {
            job_orders::LIST => self.orders.iter().filter(of_job).map(|row| row[1..].to_vec()).collect(),
            job_orders::GET => self
                .orders
                .iter()
                .filter(|row| row[..2] == p[..2])
                .map(|row| row[1..].to_vec())
                .collect(),
            job_orders::DRAFT => self
                .orders
                .iter()
                .filter(of_job)
                .filter(|row| row[4] == text("draft"))
                .map(|row| vec![row[1].clone()])
                .collect(),
            job_orders::ADD => {
                let next = self.orders.iter().filter(|row| row[2] == p[2]).count() + 1;
                let mut row = p.to_vec();
                row.extend([
                    Value::Int(i64::try_from(next).unwrap()),
                    text("draft"),
                    Value::Null,
                    Value::Null,
                ]);
                self.orders.push(row);
                Vec::new()
            }
            job_orders::SUPERSEDE => {
                for row in self
                    .orders
                    .iter_mut()
                    .filter(|row| row[2] == p[1] && row[4] == text("issued"))
                {
                    row[4] = text("superseded");
                }
                Vec::new()
            }
            job_orders::ISSUE => self
                .orders
                .iter_mut()
                .filter(|row| row[1] == p[1] && row[4] == text("draft"))
                .map(|row| {
                    row.splice(4.., [text("issued"), p[2].clone(), p[3].clone()]);
                    vec![p[1].clone()]
                })
                .collect(),
            _ => self.line_rows(sql, p),
        }
    }

    fn line_rows(&mut self, sql: &'static str, p: &[Value]) -> Vec<Row> {
        match sql {
            job_orders::LINES => self
                .lines
                .iter()
                .filter(|line| line[..2] == p[..2])
                .map(|line| vec![line[3].clone()])
                .collect(),
            job_orders::CLEAR => {
                self.lines.retain(|line| line[..2] != p[..2]);
                Vec::new()
            }
            job_orders::ADD_LINE => {
                self.lines.push(p.to_vec());
                Vec::new()
            }
            _ => self.approval_rows(sql, p),
        }
    }

    fn approval_rows(&mut self, sql: &'static str, p: &[Value]) -> Vec<Row> {
        match sql {
            approvals::REQUEST => {
                self.asked
                    .push(vec![p[1].clone(), p[2].clone(), p[3].clone(), text("pending")]);
                vec![vec![p[1].clone()]]
            }
            approvals::PENDING => self
                .asked
                .iter()
                .filter(|row| row[3] == text("pending"))
                .map(|row| row[..3].to_vec())
                .collect(),
            approvals::OPEN => self
                .asked
                .iter()
                .filter(|row| row[0] == p[1] && row[3] == text("pending"))
                .map(|row| vec![row[1].clone()])
                .collect(),
            approvals::DECIDE => {
                for row in self.asked.iter_mut().filter(|row| row[0] == p[1]) {
                    row[3] = p[2].clone();
                }
                Vec::new()
            }
            _ => Vec::new(),
        }
    }
}

impl Tx for Store {
    fn run(&mut self, sql: &'static str, p: &[Value]) -> Result<Vec<Row>, Fail> {
        Ok(match sql {
            members::FIND => {
                let Value::Text(person) = &p[1] else {
                    return Ok(Vec::new());
                };
                Role::named(person).map_or_else(Vec::new, |role| vec![vec![text(role.name())]])
            }
            AUDIT => {
                self.audits += 1;
                Vec::new()
            }
            EVENT => {
                self.events.push(p[2].clone());
                Vec::new()
            }
            jobs::GET => job_row(&p[1], &p[2]),
            _ => self.order_rows(sql, p),
        })
    }
}

type TestHost = memory::TestHost<Store>;

fn answer(openworks: &Openworks<&TestHost>, path: &str, credential: &str, body: &str) -> (u16, String) {
    let response = send(openworks, path, credential, body);
    (response.status, response.body)
}

#[test]
fn a_job_order_keeps_its_draft_revision_with_its_lines_and_prints() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();

    let draft = ok(send(
        &openworks,
        "/job-orders/save",
        "owner",
        "j1\nSet out the yard\n\nFence the gate",
    ));
    let again = ok(send(
        &openworks,
        "/job-orders/save",
        "manager",
        "j1\nSet out the yard\nClear the gate",
    ));

    assert_eq!(again, draft);
    assert_eq!(
        ok(send(&openworks, "/job-orders/get", "viewer", &draft)),
        format!(
            r#"[{{"id":"{draft}","job_id":"j1","revision":1,"status":"draft","issued_by":null,"issued_at":null,"lines":"Set out the yard\u000aClear the gate","ext":{{}}}}]"#
        )
    );
    assert_eq!(
        ok(send(&openworks, "/job-orders/print", "viewer", &draft)),
        "Job order J-2026-1, revision 1\ndraft\n1. Set out the yard\n2. Clear the gate\n"
    );
    assert_eq!(
        answer(&openworks, "/job-orders/save", "owner", "j1\n \n"),
        (400, "invalid: a job order needs a line".to_owned())
    );
    assert_eq!(answer(&openworks, "/job-orders/save", "viewer", "j1\nGo").0, 403);
    assert_eq!(answer(&openworks, "/job-orders/save", "member", "j2\nGo").0, 404);
    assert_eq!(host.store().audits, 2);
    assert_eq!(host.store().events, [text("job_orders.save"), text("job_orders.save")]);
}

#[test]
fn issuing_asks_for_the_job_order_approval_and_a_new_revision_supersedes_the_last() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    let first = ok(send(&openworks, "/job-orders/save", "member", "j1\nSet out the yard"));

    let approval = ok(send(&openworks, "/job-orders/issue", "member", &first));

    assert_eq!(
        ok(send(&openworks, "/approvals/pending", "viewer", "")),
        format!("{approval}\tjob_order\t{first}")
    );
    assert_eq!(
        answer(&openworks, "/job-orders/issue", "owner", &first),
        (400, format!("invalid: {first} is issued and does not change"))
    );
    let second = ok(send(
        &openworks,
        "/job-orders/save",
        "owner",
        "j1\nSet out the yard\nFence the gate",
    ));
    assert_ne!(second, first);
    ok(send(&openworks, "/job-orders/issue", "owner", &second));
    assert_eq!(
        ok(send(&openworks, "/job-orders/print", "viewer", &first)),
        "Job order J-2026-1, revision 1\nsuperseded, issued by member\n1. Set out the yard\n"
    );
    assert_eq!(
        ok(send(&openworks, "/job-orders/print", "viewer", &second)),
        "Job order J-2026-1, revision 2\nissued, issued by owner\n1. Set out the yard\n2. Fence the gate\n"
    );
    let listed = ok(send(&openworks, "/job-orders/list", "viewer", "j1"));
    assert!(listed.contains(r#""revision":1,"status":"superseded""#), "{listed}");
    assert!(listed.contains(r#""revision":2,"status":"issued""#), "{listed}");
    assert_eq!(
        answer(&openworks, "/approvals/decide", "member", &format!("{approval} yes")).0,
        403
    );
    assert_eq!(
        ok(send(
            &openworks,
            "/approvals/decide",
            "manager",
            &format!("{approval} yes")
        )),
        format!("{approval} approved")
    );
    assert_eq!(
        answer(&openworks, "/approvals/save-kind", "owner", "job_order"),
        (400, "invalid: a pack or the core holds the kind job_order".to_owned())
    );
}
