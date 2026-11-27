use std::cell::{Cell, RefCell};
use std::rc::Rc;

use openworks::{Files, Host, Openworks, Operation, Refused, Request, Response};
use openworks_core::{Change, Declaration, Fail, Hook, Id, Kind, Mount, Reach, Row, Tx, Value, Who};
use openworks_db::idempotency;

type Statement = (&'static str, Vec<Value>);

const T0: i64 = 1_700_000_000_000;
const DAY: i64 = 24 * 60 * 60 * 1000;

#[derive(Default)]
struct TestHost {
    committed: RefCell<Store>,
    transactions: RefCell<usize>,
    next_id: RefCell<u32>,
    clock: Cell<i64>,
}

#[derive(Clone, Default)]
struct Store {
    statements: Vec<Statement>,
    keys: Vec<Row>,
}

impl Tx for Store {
    fn run(&mut self, sql: &'static str, params: &[Value]) -> Result<Vec<Row>, Fail> {
        self.statements.push((sql, params.to_vec()));
        Ok(match sql {
            openworks_db::orgs::SHOW => vec![vec![Value::Text("First org".to_owned())]],
            openworks_db::orgs::ALL => vec![vec![Value::Text("o1".to_owned())]],
            idempotency::FIND => self
                .keys
                .iter()
                .filter(|row| row[..3] == params[..3])
                .map(|row| vec![row[3].clone()])
                .collect(),
            idempotency::SAVE => {
                self.keys.push(params.to_vec());
                Vec::new()
            }
            idempotency::PRUNE => {
                let (Value::Text(org), Value::Int(before)) = (&params[0], &params[1]) else {
                    return Err(Fail::Store("prune takes an org and a time".to_owned()));
                };
                self.keys.retain(
                    |row| !matches!((&row[0], &row[4]), (Value::Text(o), Value::Int(at)) if o == org && at < before),
                );
                Vec::new()
            }
            _ => Vec::new(),
        })
    }
}

struct NoFiles;

impl Files for NoFiles {
    fn put(&self, _: &str, _: &[u8]) -> Result<(), Fail> {
        Ok(())
    }
    fn get(&self, _: &str) -> Result<Option<Vec<u8>>, Fail> {
        Ok(None)
    }
}

impl Host for &TestHost {
    fn who(&self, credential: &str) -> Option<Who> {
        let words: &[&str] = match credential {
            "member" => &["orgs.read", "orgs.write", "notes.write"],
            "nobody" => &[],
            word if word.contains('.') => &[word][..],
            _ => return None,
        };
        let words = words.iter().map(|word| (*word).to_owned()).collect();
        Some(Who {
            person: Id("p1".to_owned()),
            org: Id("o1".to_owned()),
            words,
        })
    }

    fn transact(&self, work: &mut dyn FnMut(&mut dyn Tx) -> Result<(), Fail>) -> Result<(), Fail> {
        *self.transactions.borrow_mut() += 1;
        let mut pending = self.committed.borrow().clone();
        work(&mut pending)?;
        *self.committed.borrow_mut() = pending;
        Ok(())
    }

    fn id(&self) -> Id {
        let mut next = self.next_id.borrow_mut();
        *next += 1;
        Id(format!("id{next}"))
    }

    fn now(&self) -> i64 {
        T0 + self.clock.get()
    }

    fn files(&self) -> &dyn Files {
        &NoFiles
    }

    fn name(&self, _: &str) -> Option<String> {
        None
    }
}

impl TestHost {
    fn count(&self, sql: &str) -> usize {
        self.committed
            .borrow()
            .statements
            .iter()
            .filter(|(run, _)| *run == sql)
            .count()
    }
}

fn call(openworks: &Openworks<&TestHost>, path: &str, credential: &str, body: &str) -> Response {
    send(openworks, path, credential, None, body)
}

fn send(openworks: &Openworks<&TestHost>, path: &str, credential: &str, key: Option<&str>, body: &str) -> Response {
    openworks.handle(&Request {
        path: path.to_owned(),
        credential: credential.to_owned(),
        idempotency_key: key.map(str::to_owned),
        body: body.to_owned(),
    })
}

const NOTE: &str = "insert into works.notes (org_id, body) values ($1, $2)";
const STRAY: &str = "delete from works.notes where org_id = $1";

const ADD_NOTE: Declaration = openworks_core::write!("add", needs: "notes.write", queries: [NOTE], |who, tx, body| {
    tx.run(NOTE, &[Value::Text(who.org.0.clone()), Value::Text(body.to_owned())])?;
    Ok(String::new())
});

const STRAY_NOTE: Declaration = openworks_core::write!("stray", needs: "notes.write", queries: [NOTE], |who, tx, _| {
    tx.run(STRAY, &[Value::Text(who.org.0.clone())])?;
    Ok(String::new())
});

const UNSCOPED: Declaration = openworks_core::read!(
    "all",
    needs: "notes.read",
    queries: ["select body from works.notes"],
    |_, _, _| Ok(String::new())
);

const WHOAMI: Declaration = openworks_core::closed!("whoami", needs: "orgs.read", |who, _| Ok(who.person.0.clone()));

fn notes() -> Mount {
    Mount::new("/notes", "notes")
        .operation(&ADD_NOTE, Reach::Org)
        .operation(&STRAY_NOTE, Reach::Org)
        .operation(&WHOAMI, Reach::Org)
        .operation(&SWITCHED_OFF, Reach::Off)
        .operation(&OPEN_NOTE, Reach::Org)
}

const OPEN_NOTE: Declaration = openworks_core::create!("open", needs: "notes.write", queries: [NOTE], |who, tx, body| {
    tx.run(NOTE, &[Value::Text(who.org.0.clone()), Value::Text(body.to_owned())])?;
    Ok(format!("note {body}"))
});

const SWITCHED_OFF: Declaration = openworks_core::closed!("off", needs: "orgs.read", |_, _| Ok(String::new()));

#[test]
fn a_test_host_mounts_the_router_and_answers() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).mount(notes()).build().unwrap();

    let shown = call(&openworks, "/orgs/show", "member", "");
    assert_eq!(
        shown,
        Response {
            status: 200,
            body: "First org".to_owned()
        }
    );
    assert_eq!(call(&openworks, "/notes/whoami", "member", "").body, "p1");
    assert_eq!(call(&openworks, "/orgs/missing", "member", "").status, 404);
    assert_eq!(call(&openworks, "/notes/off", "member", "").status, 404);
    assert_eq!(call(&openworks, "/orgs/show", "stranger", "").status, 401);
}

#[test]
fn a_closed_operation_opens_no_transaction() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).mount(notes()).build().unwrap();

    assert_eq!(call(&openworks, "/notes/whoami", "member", "").status, 200);
    assert_eq!(*host.transactions.borrow(), 0);
}

#[test]
fn every_route_is_refused_without_the_word_it_needs() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();

    for operation in Operation::ALL {
        let declaration = operation.declaration();
        let path = format!("/orgs/{}", declaration.name());
        let refused = call(&openworks, &path, "nobody", "x");
        assert_eq!(
            refused,
            Response {
                status: 403,
                body: declaration.needs().0.to_owned()
            }
        );
        assert_eq!(call(&openworks, &path, declaration.needs().0, "x").status, 200);
    }
}

#[test]
fn every_write_leaves_one_audit_row_and_one_event() {
    for operation in Operation::ALL {
        let host = TestHost::default();
        let openworks = Openworks::new(&host).build().unwrap();
        let declaration = operation.declaration();
        let path = format!("/orgs/{}", declaration.name());

        assert_eq!(call(&openworks, &path, "member", "New name").status, 200);

        let trail = usize::from(declaration.kind() == Kind::Write);
        assert_eq!(host.count(openworks_db::AUDIT), trail, "{path}");
        assert_eq!(host.count(openworks_db::EVENT), trail, "{path}");
    }
}

#[test]
fn a_write_names_its_mount_key_and_operation_in_the_trail() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).mount(notes()).build().unwrap();

    assert_eq!(call(&openworks, "/notes/add", "member", "hello").status, 200);

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
    let host = TestHost::default();
    let openworks = Openworks::new(&host).mount(notes()).build().unwrap();

    assert_eq!(call(&openworks, "/orgs/rename", "member", "  ").status, 400);
    assert_eq!(call(&openworks, "/notes/stray", "member", "").status, 500);
    assert_eq!(host.committed.borrow().statements.len(), 0);
}

#[test]
fn a_query_without_org_id_fails_the_check() {
    let host = TestHost::default();
    let mount = Mount::new("/notes", "notes").operation(&UNSCOPED, Reach::Org);

    let refused = Openworks::new(&host).mount(mount).build().err();

    assert_eq!(refused, Some(Refused::Unscoped(vec!["select body from works.notes"])));
}

#[test]
fn every_compiled_query_is_scoped_by_org() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).mount(notes()).build().unwrap();

    assert_eq!(openworks.check(), Ok(()));
}

#[test]
fn a_path_is_mounted_once() {
    let host = TestHost::default();

    let refused = Openworks::new(&host).mount(Operation::mount()).build().err();

    assert_eq!(refused, Some(Refused::DuplicatePath("/orgs/show".to_owned())));
}

#[test]
fn migrate_runs_every_statement_and_again_does_the_same() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();

    openworks.migrate().unwrap();
    let once = host.committed.borrow().statements.clone();
    openworks.migrate().unwrap();

    assert_eq!(once.len(), 6);
    assert!(once.iter().all(|(sql, _)| sql.contains("if not exists")));
    assert_eq!(host.committed.borrow().statements[once.len()..], once[..]);
}

#[test]
fn migrate_gives_every_event_a_sequence_number() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();

    openworks.migrate().unwrap();

    let statements = &host.committed.borrow().statements;
    assert!(statements.iter().any(|(sql, _)| sql.starts_with(
        "alter table works.events add column if not exists seq bigint generated always as identity unique"
    )));
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
    let host = TestHost::default();
    let calls = Rc::new(RefCell::new(Vec::new()));
    let veto = Recorder {
        calls: calls.clone(),
        refuses: true,
        name: "veto",
        ..Recorder::default()
    };
    let openworks = Openworks::new(&host).hook(veto).build().unwrap();

    let refused = call(&openworks, "/orgs/rename", "member", "New name");

    assert_eq!(refused.status, 400);
    assert_eq!(host.committed.borrow().statements.len(), 0);
    assert_eq!(*calls.borrow(), ["veto before orgs.rename"]);
}

#[test]
fn after_commit_hooks_run_in_order_and_a_failure_is_retried() {
    let host = TestHost::default();
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

    assert_eq!(call(&openworks, "/orgs/rename", "member", "New name").status, 200);
    assert_eq!(call(&openworks, "/orgs/show", "member", "").status, 200);

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
    let host = TestHost::default();
    let openworks = Openworks::new(&host).mount(notes()).build().unwrap();

    for key in [None, Some("")] {
        let refused = send(&openworks, "/notes/open", "member", key, "hello");
        assert_eq!(refused.status, 400);
    }
    assert_eq!(host.committed.borrow().statements.len(), 0);
}

#[test]
fn a_create_repeated_under_its_key_runs_once_and_answers_the_same() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).mount(notes()).build().unwrap();

    let first = send(&openworks, "/notes/open", "member", Some("k1"), "hello");
    let again = send(&openworks, "/notes/open", "member", Some("k1"), "hello");
    let other = send(&openworks, "/notes/open", "member", Some("k2"), "hello");

    assert_eq!(first.body, "note hello");
    assert_eq!(again, first);
    assert_eq!(other.status, 200);
    assert_eq!(host.count(NOTE), 2);
    assert_eq!(host.count(openworks_db::AUDIT), 2);
    assert_eq!(host.count(openworks_db::EVENT), 2);
}

#[test]
fn running_due_work_twice_does_what_once_did() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).mount(notes()).build().unwrap();
    assert_eq!(send(&openworks, "/notes/open", "member", Some("old"), "a").status, 200);
    host.clock.set(2 * DAY);
    assert_eq!(send(&openworks, "/notes/open", "member", Some("new"), "b").status, 200);

    openworks.run_due(T0 + 2 * DAY).unwrap();
    let once = host.committed.borrow().keys.clone();
    openworks.run_due(T0 + 2 * DAY).unwrap();

    assert_eq!(host.committed.borrow().keys, once);
    assert_eq!(once.len(), 1);
    assert_eq!(once[0][2], Value::Text("new".to_owned()));
    assert_eq!(host.count(idempotency::PRUNE), 2);
}
