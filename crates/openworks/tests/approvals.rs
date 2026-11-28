mod common;

use common::{OWNER, TestHost, call, send, staffed, staffed_with};
use openworks::{Openworks, Response};
use openworks_core::{Approval, Fail, Pack, Row, Tx, Value, Word};
use openworks_db::{approvals, attachments};

#[derive(Clone, Default)]
struct Ledger {
    kinds: Vec<Row>,
    approvals: Vec<Row>,
    files: Vec<Row>,
}

impl Tx for Ledger {
    fn run(&mut self, sql: &'static str, params: &[Value]) -> Result<Vec<Row>, Fail> {
        Ok(self.approval_rows(sql, params).unwrap_or_default())
    }
}

fn pick_from(rows: &[Row], key: &Value, column: usize) -> Vec<Row> {
    rows.iter()
        .filter(|row| row[0] == *key)
        .map(|row| vec![row[column].clone()])
        .collect()
}

fn live_state(row: &Row) -> bool {
    row[4] == Value::Text("pending".to_owned())
}

impl Ledger {
    fn approval_rows(&mut self, sql: &'static str, params: &[Value]) -> Option<Vec<Row>> {
        let p = |at: usize| params[at].clone();
        let pick = |rows: &mut dyn Iterator<Item = &Row>, columns: [usize; 7]| -> Vec<Row> {
            rows.map(|row| columns.iter().map(|&at| row[at].clone()).collect())
                .collect()
        };
        Some(match sql {
            approvals::KIND => pick_from(&self.kinds, &p(1), 1),
            attachments::FIND => pick_from(&self.files, &p(1), 0),
            approvals::PENDING => pick(
                &mut self.approvals.iter().filter(|row| live_state(row)),
                [1, 2, 3, 5, 7, 8, 10],
            ),
            approvals::RECENT => {
                let mut decided: Vec<&Row> = self.approvals.iter().filter(|row| row[12] != Value::Null).collect();
                decided.sort_by_key(|row| {
                    std::cmp::Reverse(match row[12] {
                        Value::Int(at) => at,
                        _ => 0,
                    })
                });
                pick(&mut decided.into_iter(), [1, 2, 3, 4, 11, 13, 14])
            }
            _ => return self.approval_writes(sql, params),
        })
    }

    fn approval_writes(&mut self, sql: &'static str, params: &[Value]) -> Option<Vec<Row>> {
        let p = |at: usize| params[at].clone();
        let text = |text: &str| Value::Text(text.to_owned());
        let live = |row: &Row| row[1] == p(1) && row[4] == text("pending");
        Some(match sql {
            approvals::SAVE_KIND => {
                self.kinds.retain(|kind| kind[0] != p(1));
                self.kinds.push(vec![p(1), p(2)]);
                Vec::new()
            }
            approvals::REQUEST => {
                let held = [text("pending"), text("approved")];
                if self
                    .approvals
                    .iter()
                    .any(|row| row[2..4] == params[2..4] && held.contains(&row[4]))
                {
                    return Some(Vec::new());
                }
                let mut row = [&params[..4], &[text("pending")], &params[4..]].concat();
                row.resize(15, Value::Null);
                self.approvals.push(row);
                vec![vec![p(1)]]
            }
            approvals::OPEN => self
                .approvals
                .iter()
                .filter(|row| live(row))
                .map(|row| vec![row[2].clone()])
                .collect(),
            approvals::PROPOSE | approvals::DECIDE => {
                for row in self.approvals.iter_mut().filter(|row| live(row)) {
                    if sql == approvals::PROPOSE {
                        row.splice(8..11, params[2..5].iter().cloned());
                    } else {
                        row[4] = p(2);
                        row.splice(11..15, params[3..7].iter().cloned());
                    }
                }
                Vec::new()
            }
            approvals::RESET => self
                .approvals
                .iter_mut()
                .filter(|row| row[1] == p(1) && row[4] == text("approved"))
                .map(|row| {
                    row[4] = text("reset");
                    vec![p(1)]
                })
                .collect(),
            _ => return None,
        })
    }
}

fn seed(host: &TestHost<Ledger>) {
    let mut committed = host.committed.borrow_mut();
    let store = &mut committed.area;
    let text = |text: &str| Value::Text(text.to_owned());
    store.kinds = vec![vec![text("visit"), text("no")], vec![text("handover"), text("yes")]];
    store.files = vec![vec![text("f1")]];
    for (id, subject, state, decided_at) in [
        ("a1", "job-1", "pending", Value::Null),
        ("a2", "job-2", "approved", Value::Int(0)),
    ] {
        let asked = ["o1", id, "visit", subject, state, "person:member@o.test"].map(text);
        let mut row = [&asked[..], &[Value::Int(0), text("seeded")]].concat();
        row.resize(15, Value::Null);
        row[12] = decided_at;
        store.approvals.push(row);
    }
}

const MANAGER: &str = "manager@o.test";

fn ask(openworks: &Openworks<&TestHost<Ledger>>, key: &str, body: &str) -> Response {
    send(openworks, "/approvals/request", "member@o.test", Some(key), body)
}

fn decide(openworks: &Openworks<&TestHost<Ledger>>, who: &str, body: &str) -> String {
    call(openworks, "/approvals/decide", who, body).body
}

fn listed(openworks: &Openworks<&TestHost<Ledger>>, read: &str) -> Vec<String> {
    let body = call(openworks, &format!("/approvals/{read}"), "viewer@o.test", "").body;
    body.lines().map(str::to_owned).collect()
}

#[test]
fn a_rejected_approval_is_asked_for_again() {
    let host = TestHost::<Ledger>::default();
    let openworks = staffed(&host, None);
    seed(&host);

    let first = ask(&openworks, "r1", "visit job-9\nsite visit").body;
    let line = format!("{first}\tvisit\tjob-9\tperson:member@o.test\tsite visit\t\t");
    assert_eq!(listed(&openworks, "pending")[1], line);
    assert_eq!(
        decide(&openworks, MANAGER, &format!("{first} no")),
        "invalid: a no needs a note"
    );
    assert_eq!(
        decide(&openworks, MANAGER, &format!("{first} no\ntoo early")),
        format!("{first} rejected")
    );
    assert_eq!(decide(&openworks, MANAGER, &format!("{first} yes")), "not found");
    let rejected = format!("{first}\tvisit\tjob-9\trejected\tperson:{MANAGER}\ttoo early\t");
    assert_eq!(listed(&openworks, "recent")[0], rejected);

    let again = ask(&openworks, "r2", "visit job-9").body;
    assert_ne!(again, first);
    assert_eq!(listed(&openworks, "pending").len(), 2);
    assert_eq!(host.count(openworks_db::AUDIT), 3);
    assert_eq!(host.count(openworks_db::EVENT), 3);
}

#[test]
fn an_approved_approval_holds_until_an_owner_resets_it() {
    let host = TestHost::<Ledger>::default();
    let openworks = staffed(&host, None);
    seed(&host);

    assert_eq!(decide(&openworks, MANAGER, "a1 yes"), "a1 approved");
    assert_eq!(ask(&openworks, "r1", "visit job-1").status, 400);
    assert_eq!(
        call(&openworks, "/approvals/reset", MANAGER, "a1").body,
        "needs settings"
    );
    assert_eq!(call(&openworks, "/approvals/reset", OWNER, "a9").status, 400);
    assert_eq!(call(&openworks, "/approvals/reset", OWNER, "a1").status, 200);
    assert_eq!(call(&openworks, "/approvals/reset", OWNER, "a1").status, 400);
    assert_eq!(ask(&openworks, "r2", "visit job-1").status, 200);
}

#[test]
fn a_kind_that_asks_for_a_signature_takes_a_yes_only_with_a_stored_file() {
    let host = TestHost::<Ledger>::default();
    let openworks = staffed(&host, None);
    seed(&host);
    let id = ask(&openworks, "r", "handover job-1").body;

    for body in [
        format!("{id} yes"),
        format!("{id} yes f9"),
        format!("{id} no f1\nnot yet"),
    ] {
        assert!(decide(&openworks, MANAGER, &body).starts_with("invalid"), "{body}");
    }
    assert_eq!(
        decide(&openworks, MANAGER, &format!("{id} yes f1\nsigned")),
        format!("{id} approved")
    );
    let signed = format!("{id}\thandover\tjob-1\tapproved\tperson:{MANAGER}\tsigned\tf1");
    assert_eq!(listed(&openworks, "recent")[0], signed);
}

#[test]
fn an_assistant_proposes_a_decision_and_never_makes_one() {
    let host = TestHost::<Ledger>::default();
    let openworks = staffed(&host, None);
    seed(&host);

    assert_eq!(
        decide(&openworks, "assistant:manager@o.test", "a1 yes\nlooks done"),
        "a1 proposed yes"
    );
    let proposed = "a1\tvisit\tjob-1\tperson:member@o.test\tseeded\tyes\tlooks done";
    assert_eq!(listed(&openworks, "pending"), [proposed]);
    assert_eq!(host.count(approvals::DECIDE), 0);
    assert_eq!(host.count(openworks_db::AUDIT), 1);
    assert_eq!(host.count(openworks_db::EVENT), 1);

    assert_eq!(decide(&openworks, MANAGER, "a1 yes"), "a1 approved");
    assert_eq!(listed(&openworks, "pending").len(), 0);
}

#[test]
fn a_pack_kind_shares_the_engine_and_asks_for_its_own_word() {
    let host = TestHost::<Ledger>::default();
    let kind = Approval {
        key: "pack.check",
        needs: Word::Settings,
        signature: false,
    };
    let openworks = staffed_with(
        &host,
        None,
        Pack {
            approvals: vec![kind],
            ..Pack::default()
        },
    );
    seed(&host);

    let id = ask(&openworks, "r", "pack.check thing-7").body;
    assert_eq!(decide(&openworks, MANAGER, &format!("{id} yes")), "needs settings");
    assert_eq!(
        decide(&openworks, OWNER, &format!("{id} yes")),
        format!("{id} approved")
    );
    assert_eq!(
        call(&openworks, "/approvals/save-kind", OWNER, "pack.check").status,
        400
    );
}

#[test]
fn a_company_adds_its_own_kind_and_an_unknown_kind_is_refused() {
    let host = TestHost::<Ledger>::default();
    let openworks = staffed(&host, None);
    seed(&host);

    assert_eq!(
        ask(&openworks, "r1", "sign-off job-1").body,
        "invalid: no approval kind sign-off"
    );
    assert_eq!(
        call(&openworks, "/approvals/save-kind", MANAGER, "sign-off").status,
        403
    );
    assert_eq!(
        call(&openworks, "/approvals/save-kind", OWNER, "sign-off signature").body,
        "sign-off"
    );
    let id = ask(&openworks, "r2", "sign-off job-1").body;
    assert!(decide(&openworks, MANAGER, &format!("{id} yes")).starts_with("invalid"));
    assert_eq!(
        decide(&openworks, MANAGER, &format!("{id} yes f1")),
        format!("{id} approved")
    );
    assert_eq!(
        ask(&openworks, "r3", "visit job-1\ntwo\tcells").body,
        "invalid: a note is one line"
    );
}
