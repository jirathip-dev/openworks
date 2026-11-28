mod memory;
mod table;

use memory::{ok, send};
use openworks::Openworks;
use openworks_core::{Fail, Row, Tx, Value};
use openworks_db::{teams, workers};
use table::{Trail, change, select, text};

#[derive(Clone, Default)]
struct Store {
    teams: Vec<Row>,
    workers: Vec<Row>,
    trail: Trail,
}

impl Tx for Store {
    fn run(&mut self, sql: &'static str, p: &[Value]) -> Result<Vec<Row>, Fail> {
        let keyed = |row: &Row| row[0] == p[0] && row[1] == p[1];
        if let Some(rows) = self.trail.run(sql, p) {
            return Ok(rows);
        }
        Ok(match sql {
            teams::LIST => select(&self.teams, |row| row[0] == p[0]),
            teams::GET => select(&self.teams, keyed),
            teams::ADD => {
                self.teams.push(p.to_vec());
                Vec::new()
            }
            teams::SAVE => change(&mut self.teams, keyed, |row| row[2] = p[2].clone()),
            workers::LIST => select(&self.workers, |row| row[0] == p[0]),
            workers::GET => select(&self.workers, keyed),
            workers::ME => select(&self.workers, |row| row[0] == p[0] && row[6] == p[1]),
            workers::ADD => {
                self.workers.push([p, &[Value::Int(1)]].concat());
                Vec::new()
            }
            workers::SAVE => change(&mut self.workers, keyed, |row| row[2..7].clone_from_slice(&p[2..7])),
            workers::SET_ACTIVE => change(&mut self.workers, keyed, |row| row[7] = p[2].clone()),
            _ => Vec::new(),
        })
    }
}

type TestHost = memory::TestHost<Store>;

fn worker(id: &str, fields: &str, active: u8) -> String {
    format!(r#"{{"id":"{id}",{fields},"active":{active},"ext":{{}}}}"#)
}

#[test]
fn a_team_is_saved_listed_and_renamed() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();

    let team = ok(send(&openworks, "/teams/save", "owner", "\nCrew A"));
    assert_eq!(
        ok(send(&openworks, "/teams/save", "owner", &format!("{team}\nCrew B"))),
        team
    );

    assert_eq!(
        ok(send(&openworks, "/teams/list", "viewer", "")),
        format!(r#"[{{"id":"{team}","name":"Crew B","ext":{{}}}}]"#)
    );
    assert_eq!(send(&openworks, "/teams/save", "owner", "nobody\nX").status, 404);
    assert_eq!(send(&openworks, "/teams/save", "owner", "\n ").status, 400);
    assert_eq!(send(&openworks, "/teams/save", "viewer", "\nCrew C").status, 403);
    assert_eq!(ok(send(&openworks, "/teams/list", "other", "")), "[]");
    assert_eq!(host.store().trail.audits, 2);
    assert_eq!(host.store().trail.events[0], (text("teams.save"), text("\nCrew A")));
}

#[test]
fn a_worker_is_own_staff_or_an_outside_company_s_in_a_team_with_an_optional_user() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    let team = ok(send(&openworks, "/teams/save", "owner", "\nCrew A"));

    let ada = ok(send(
        &openworks,
        "/workers/save",
        "owner",
        &format!("\nAda\nForeman\n{team}\n\nada"),
    ));
    let bo = ok(send(&openworks, "/workers/save", "owner", "\nBo\nHelper\n\nHire Co\n"));

    let own = r#""name":"Ada","title":"Foreman","team_id":"id1","company":null,"person_id":"ada""#;
    let outside = r#""name":"Bo","title":"Helper","team_id":null,"company":"Hire Co","person_id":null"#;
    assert_eq!(
        ok(send(&openworks, "/workers/list", "viewer", "")),
        format!("[{},{}]", worker(&ada, own, 1), worker(&bo, outside, 1))
    );
    assert_eq!(
        ok(send(&openworks, "/workers/get", "viewer", &bo)),
        format!("[{}]", worker(&bo, outside, 1))
    );
    assert_eq!(
        ok(send(
            &openworks,
            "/workers/save",
            "owner",
            &format!("{bo}\nBo\nRigger\n{team}\n\n")
        )),
        bo
    );
    assert!(
        ok(send(&openworks, "/workers/get", "viewer", &bo))
            .contains(r#""title":"Rigger","team_id":"id1","company":null"#)
    );

    assert_eq!(send(&openworks, "/workers/save", "owner", "\nCy\n\nnoteam").status, 404);
    assert_eq!(send(&openworks, "/workers/save", "owner", "nobody\nCy").status, 404);
    assert_eq!(send(&openworks, "/workers/save", "owner", "\n\nForeman").status, 400);
    assert_eq!(send(&openworks, "/workers/save", "viewer", "\nCy").status, 403);
    assert_eq!(send(&openworks, "/workers/get", "viewer", "nobody").status, 404);
    assert_eq!(send(&openworks, "/workers/get", "other", &ada).status, 404);
    assert_eq!(ok(send(&openworks, "/workers/list", "other", "")), "[]");
    assert_eq!(host.store().trail.audits, 4);
    assert_eq!(host.store().trail.events.len(), 4);
}

#[test]
fn workers_me_answers_the_signed_in_person_s_worker_and_a_person_is_one_worker() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    let ada = ok(send(&openworks, "/workers/save", "owner", "\nAda\n\n\n\nviewer"));

    assert_eq!(
        ok(send(&openworks, "/workers/me", "viewer", "")),
        format!(
            "[{}]",
            worker(
                &ada,
                r#""name":"Ada","title":"","team_id":null,"company":null,"person_id":"viewer""#,
                1
            )
        )
    );
    assert_eq!(send(&openworks, "/workers/me", "owner", "").status, 404);
    assert_eq!(
        ok(send(
            &openworks,
            "/workers/save",
            "owner",
            &format!("{ada}\nAda L\n\n\n\nviewer")
        )),
        ada
    );
    let taken = send(&openworks, "/workers/save", "owner", "\nImposter\n\n\n\nviewer");
    assert_eq!(
        (taken.status, taken.body.as_str()),
        (400, "invalid: person viewer is another worker")
    );
    assert_eq!(host.store().workers.len(), 1);
}

#[test]
fn a_worker_is_set_inactive_and_active_again() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    let ada = ok(send(&openworks, "/workers/save", "owner", "\nAda"));

    assert_eq!(
        ok(send(&openworks, "/workers/set-active", "owner", &format!("{ada}\nno"))),
        ada
    );
    assert!(ok(send(&openworks, "/workers/get", "viewer", &ada)).contains(r#""active":0,"#));
    ok(send(&openworks, "/workers/set-active", "owner", &format!("{ada}\nyes")));
    assert!(ok(send(&openworks, "/workers/get", "viewer", &ada)).contains(r#""active":1,"#));

    assert_eq!(
        send(&openworks, "/workers/set-active", "owner", &format!("{ada}\nmaybe")).status,
        400
    );
    assert_eq!(
        send(&openworks, "/workers/set-active", "other", &format!("{ada}\nno")).status,
        404
    );
    assert_eq!(
        send(&openworks, "/workers/set-active", "viewer", &format!("{ada}\nno")).status,
        403
    );
    assert_eq!(
        host.store().trail.events.last().unwrap(),
        &(text("workers.set-active"), text(&format!("{ada}\nyes")))
    );
}
