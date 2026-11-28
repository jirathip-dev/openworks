mod common;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use common::{Member, OWNER, T0, TestHost, call, send, staffed};
use openworks::{Openworks, Refused, Response};
use openworks_core::{Change, Declaration, Fail, Hook, Mount, Reach, Role, Scope, Value, Word};
use openworks_db::{idempotency, members};

const DAY: i64 = 24 * 60 * 60 * 1000;

fn email(role: Role) -> String {
    format!("{}@o.test", role.name())
}

const NOTE: &str = "insert into works.notes (org_id, body) values ($1, $2)";
const STRAY: &str = "delete from works.notes where org_id = $1";

const ADD_NOTE: Declaration = openworks_core::write!("add", needs: Write, queries: [NOTE], |call, tx| {
    tx.run(NOTE, &[Value::Text(call.who.org.0.clone()), Value::Text(call.body.to_owned())])?;
    Ok(String::new())
});

const STRAY_NOTE: Declaration = openworks_core::write!("stray", needs: Write, queries: [NOTE], |call, tx| {
    tx.run(STRAY, &[Value::Text(call.who.org.0.clone())])?;
    Ok(String::new())
});

const UNSCOPED: Declaration = openworks_core::read!(
    "all",
    needs: Read,
    queries: ["select body from works.notes"],
    |_, _| Ok(String::new())
);

const READ_NOTES: Declaration = openworks_core::read!(
    "read",
    needs: Read,
    queries: ["select body from works.notes where org_id = $1"],
    |_, _| Ok(String::new())
);

const WHOAMI: Declaration = openworks_core::closed!("whoami", needs: Read, |who, _| Ok(who.person.0.clone()));

fn notes() -> Mount {
    Mount::new("/notes", "notes")
        .operation(&READ_NOTES, Reach::Org)
        .operation(&ADD_NOTE, Reach::Org)
        .operation(&STRAY_NOTE, Reach::Org)
        .operation(&WHOAMI, Reach::Org)
        .operation(&SWITCHED_OFF, Reach::Off)
        .operation(&OPEN_NOTE, Reach::Org)
}

const OPEN_NOTE: Declaration = openworks_core::create!("open", needs: Write, queries: [NOTE], |call, tx| {
    tx.run(NOTE, &[Value::Text(call.who.org.0.clone()), Value::Text(call.body.to_owned())])?;
    Ok(format!("note {}", call.body))
});

const SWITCHED_OFF: Declaration = openworks_core::closed!("off", needs: Read, |_, _| Ok(String::new()));

#[test]
fn the_role_table_grants_every_word() {
    use Scope::{All, Own};
    let table = [
        (Role::Owner, [Some(All), Some(All), Some(All), Some(All), Some(All)]),
        (Role::Manager, [Some(All), Some(All), Some(All), None, None]),
        (Role::Member, [Some(Own), Some(Own), None, None, None]),
        (Role::Viewer, [Some(All), None, None, None, None]),
    ];
    assert_eq!(
        Word::ALL,
        [Word::Read, Word::Write, Word::Approve, Word::Members, Word::Settings]
    );
    assert_eq!(table.map(|(role, _)| role), Role::ALL);
    for (role, scopes) in table {
        for (word, scope) in Word::ALL.into_iter().zip(scopes) {
            assert_eq!(role.grants(word), scope, "{} {}", role.name(), word.name());
        }
    }
}

#[test]
fn the_role_table_holds_for_every_declared_operation() {
    for mount in openworks::mounts() {
        for &(declaration, _) in mount.operations() {
            let path = mount.path(declaration);
            let needs = format!("needs {}", declaration.needs().name());
            for role in Role::ALL {
                let host: TestHost = TestHost::default();
                let openworks = staffed(&host, None);

                let answered = send(&openworks, &path, &email(role), Some("k"), "");

                if role.grants(declaration.needs()) == Some(Scope::All) {
                    assert_ne!(answered.body, needs, "{path} as {}", role.name());
                } else if role.grants(declaration.needs()).is_none() {
                    assert_eq!(
                        (answered.status, answered.body),
                        (403, needs.clone()),
                        "{path} as {}",
                        role.name()
                    );
                    assert!(
                        host.committed.borrow().statements.is_empty(),
                        "{path} as {}",
                        role.name()
                    );
                }
            }
        }
    }
}

#[test]
fn a_test_host_mounts_the_router_and_answers() {
    let host: TestHost = TestHost::default();
    let openworks = staffed(&host, Some(notes()));

    let listed = call(&openworks, "/orgs/list", "viewer@o.test", "");
    assert_eq!(
        listed,
        Response {
            status: 200,
            body: r#"[{"id":"o1","name":"First \"org\"","ext":{}}]"#.to_owned()
        }
    );
    assert_eq!(
        call(&openworks, "/notes/whoami", "viewer@o.test", "").body,
        "person:viewer@o.test"
    );
    assert_eq!(call(&openworks, "/orgs/missing", OWNER, "").status, 404);
    assert_eq!(call(&openworks, "/notes/off", OWNER, "").status, 404);
    assert_eq!(call(&openworks, "/orgs/list", "stranger", "").status, 401);
}

#[test]
fn a_closed_operation_runs_no_query_but_the_sign_in() {
    let host: TestHost = TestHost::default();
    let openworks = staffed(&host, Some(notes()));

    assert_eq!(call(&openworks, "/notes/whoami", OWNER, "").status, 200);
    assert_eq!(*host.transactions.borrow(), 1);
    let committed = host.committed.borrow();
    assert!(
        committed.statements.iter().all(|(sql, _)| *sql == members::FIND),
        "{:?}",
        committed.statements
    );
}

#[test]
fn every_write_leaves_one_audit_row_and_one_event() {
    for (path, trail) in [
        ("/notes/read", 0),
        ("/notes/add", 1),
        ("/notes/open", 1),
        ("/notes/whoami", 0),
    ] {
        let host: TestHost = TestHost::default();
        let openworks = staffed(&host, Some(notes()));

        assert_eq!(send(&openworks, path, OWNER, Some("k"), "hello").status, 200, "{path}");

        assert_eq!(host.count(openworks_db::AUDIT), trail, "{path}");
        assert_eq!(host.count(openworks_db::EVENT), trail, "{path}");
    }
}

#[test]
fn a_write_names_its_mount_key_and_operation_in_the_trail() {
    let host: TestHost = TestHost::default();
    let openworks = staffed(&host, Some(notes()));

    assert_eq!(call(&openworks, "/notes/add", "member@o.test", "hello").status, 200);

    let committed = &host.committed.borrow().statements;
    let named = Value::Text("notes.add".to_owned());
    let audit = committed.iter().find(|(sql, _)| *sql == openworks_db::AUDIT).unwrap();
    let event = committed.iter().find(|(sql, _)| *sql == openworks_db::EVENT).unwrap();
    assert_eq!(audit.1[3], named);
    assert_eq!(event.1[2], named);
    assert_eq!(event.1[3], Value::Text("hello".to_owned()));
}

#[test]
fn a_failed_write_leaves_nothing() {
    let host: TestHost = TestHost::default();
    let openworks = staffed(&host, Some(notes()));

    assert_eq!(call(&openworks, "/orgs/rename", OWNER, "  ").status, 400);
    assert_eq!(call(&openworks, "/notes/stray", OWNER, "").status, 500);
    assert_eq!(host.committed.borrow().statements.len(), 0);
}

#[test]
fn a_query_without_org_id_fails_the_check() {
    let host: TestHost = TestHost::default();
    let mount = Mount::new("/notes", "notes").operation(&UNSCOPED, Reach::Org);

    let refused = Openworks::new(&host).mount(mount).build().err();

    assert_eq!(refused, Some(Refused::Unscoped(vec!["select body from works.notes"])));
}

#[test]
fn every_compiled_query_is_scoped_by_org() {
    let host: TestHost = TestHost::default();
    let openworks = Openworks::new(&host).mount(notes()).build().unwrap();

    assert_eq!(openworks.check(), Ok(()));
}

#[test]
fn a_path_is_mounted_once() {
    let host: TestHost = TestHost::default();
    let first = openworks::mounts().remove(0);
    let path = first.path(first.operations()[0].0);

    let refused = Openworks::new(&host).mount(first).build().err();

    assert_eq!(refused, Some(Refused::DuplicatePath(path)));
}

#[test]
fn migrate_runs_every_statement_and_again_does_the_same() {
    let host: TestHost = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    let statements = openworks_db::MIGRATIONS
        .iter()
        .flat_map(|(_, sql)| sql.split(';'))
        .filter(|statement| !statement.trim().is_empty())
        .count();

    openworks.migrate().unwrap();
    let once = host.committed.borrow().statements.clone();
    openworks.migrate().unwrap();

    assert_eq!(once.len(), statements);
    assert!(once.iter().all(|(sql, _)| sql.contains("if not exists")));
    assert_eq!(host.committed.borrow().statements[once.len()..], once[..]);
}

#[test]
fn migrate_gives_every_event_a_sequence_number() {
    let host: TestHost = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();

    openworks.migrate().unwrap();

    let statements = &host.committed.borrow().statements;
    assert!(statements.iter().any(|(sql, _)| sql.starts_with(
        "alter table works.events add column if not exists seq bigint generated always as identity unique"
    )));
}

fn owned() -> TestHost {
    let host: TestHost = TestHost::default();
    host.committed.borrow_mut().members.push(Member {
        email: OWNER.to_owned(),
        person: Some(format!("person:{OWNER}")),
        role: Role::Owner.name().to_owned(),
    });
    host
}

#[derive(Clone, Default)]
struct Recorder {
    calls: Rc<RefCell<Vec<String>>>,
    refuses: bool,
    fails_after_commit: Rc<Cell<usize>>,
    name: &'static str,
}

impl Hook for Recorder {
    fn before_commit(&self, change: &Change) -> Result<(), Fail> {
        self.calls
            .borrow_mut()
            .push(format!("{} before {}", self.name, change.operation));
        if self.refuses {
            return Err(Fail::Invalid("refused".to_owned()));
        }
        Ok(())
    }

    fn after_commit(&self, change: &Change) -> Result<(), Fail> {
        self.calls
            .borrow_mut()
            .push(format!("{} after {}", self.name, change.operation));
        let left = self.fails_after_commit.get();
        if left > 0 {
            self.fails_after_commit.set(left - 1);
            return Err(Fail::Store("down".to_owned()));
        }
        Ok(())
    }
}

#[test]
fn a_hook_that_refuses_rolls_the_change_back() {
    let host = owned();
    let calls = Rc::new(RefCell::new(Vec::new()));
    let veto = Recorder {
        calls: calls.clone(),
        refuses: true,
        name: "veto",
        ..Recorder::default()
    };
    let openworks = Openworks::new(&host).hook(veto).build().unwrap();

    let refused = call(&openworks, "/orgs/rename", OWNER, "New name");

    assert_eq!(refused.status, 400);
    assert_eq!(host.committed.borrow().statements.len(), 0);
    assert_eq!(*calls.borrow(), ["veto before orgs.rename"]);
}

#[test]
fn after_commit_hooks_run_in_order_and_a_failure_is_retried() {
    let host = owned();
    let calls = Rc::new(RefCell::new(Vec::new()));
    let flaky = Recorder {
        calls: calls.clone(),
        fails_after_commit: Rc::new(Cell::new(2)),
        name: "flaky",
        ..Recorder::default()
    };
    let steady = Recorder {
        calls: calls.clone(),
        name: "steady",
        ..Recorder::default()
    };
    let openworks = Openworks::new(&host).hook(flaky).hook(steady).build().unwrap();

    assert_eq!(call(&openworks, "/orgs/rename", OWNER, "New name").status, 200);
    assert_eq!(call(&openworks, "/orgs/list", OWNER, "").status, 200);

    assert_eq!(host.count(openworks_db::EVENT), 1);
    assert_eq!(
        *calls.borrow(),
        [
            "flaky before orgs.rename",
            "steady before orgs.rename",
            "flaky after orgs.rename",
            "flaky after orgs.rename",
            "flaky after orgs.rename",
            "steady after orgs.rename",
        ]
    );
}

#[test]
fn a_create_needs_an_idempotency_key() {
    let host: TestHost = TestHost::default();
    let openworks = staffed(&host, Some(notes()));

    for key in [None, Some("")] {
        let refused = send(&openworks, "/notes/open", OWNER, key, "hello");
        assert_eq!(refused.status, 400);
    }
    assert_eq!(host.committed.borrow().statements.len(), 0);
}

#[test]
fn a_create_repeated_under_its_key_runs_once_and_answers_the_same() {
    let host: TestHost = TestHost::default();
    let openworks = staffed(&host, Some(notes()));

    let first = send(&openworks, "/notes/open", OWNER, Some("k1"), "hello");
    let again = send(&openworks, "/notes/open", OWNER, Some("k1"), "hello");
    let other = send(&openworks, "/notes/open", OWNER, Some("k2"), "hello");

    assert_eq!(first.body, "note hello");
    assert_eq!(again, first);
    assert_eq!(other.status, 200);
    assert_eq!(host.count(NOTE), 2);
    assert_eq!(host.count(openworks_db::AUDIT), 2);
    assert_eq!(host.count(openworks_db::EVENT), 2);
}

#[test]
fn running_due_work_twice_does_what_once_did() {
    let host: TestHost = TestHost::default();
    let openworks = staffed(&host, Some(notes()));
    assert_eq!(send(&openworks, "/notes/open", OWNER, Some("old"), "a").status, 200);
    host.clock.set(2 * DAY);
    assert_eq!(send(&openworks, "/notes/open", OWNER, Some("new"), "b").status, 200);

    openworks.run_due(T0 + 2 * DAY).unwrap();
    let once = host.committed.borrow().keys.clone();
    openworks.run_due(T0 + 2 * DAY).unwrap();

    assert_eq!(host.committed.borrow().keys, once);
    assert_eq!(once.len(), 1);
    assert_eq!(once[0][2], Value::Text("new".to_owned()));
    assert_eq!(host.count(idempotency::PRUNE), 2);
}

#[test]
fn the_openapi_document_names_every_route_with_its_word_and_types_ext() {
    let host: TestHost = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();

    let document = openworks.openapi();

    for mount in openworks::mounts() {
        for &(declaration, _) in mount.operations() {
            let path = format!(
                r#""{}":{{"post":{{"x-needs":"{}","#,
                mount.path(declaration),
                declaration.needs().name()
            );
            assert!(document.contains(&path), "{document}");
        }
    }
    let orgs =
        r#""orgs":{"type":"object","properties":{"id":{"type":"string"},"ext":{"type":"object","properties":{}}}}"#;
    assert!(document.contains(orgs), "{document}");
}
