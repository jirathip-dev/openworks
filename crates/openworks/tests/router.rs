use std::cell::RefCell;

use openworks::{Files, Host, Openworks, Operation, Refused, Request, Response};
use openworks_core::{Declaration, Fail, Id, Kind, Mount, Reach, Row, Tx, Value, Who};

type Statement = (&'static str, Vec<Value>);

#[derive(Default)]
struct TestHost {
    committed: RefCell<Vec<Statement>>,
    transactions: RefCell<usize>,
    next_id: RefCell<u32>,
}

struct Log<'a>(&'a mut Vec<Statement>);

impl Tx for Log<'_> {
    fn run(&mut self, sql: &'static str, params: &[Value]) -> Result<Vec<Row>, Fail> {
        self.0.push((sql, params.to_vec()));
        if sql == openworks_db::orgs::SHOW {
            return Ok(vec![vec![Value::Text("First org".to_owned())]]);
        }
        Ok(Vec::new())
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
        let mut pending = Vec::new();
        work(&mut Log(&mut pending))?;
        self.committed.borrow_mut().extend(pending);
        Ok(())
    }

    fn id(&self) -> Id {
        let mut next = self.next_id.borrow_mut();
        *next += 1;
        Id(format!("id{next}"))
    }

    fn now(&self) -> i64 {
        1_700_000_000_000
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
        self.committed.borrow().iter().filter(|(run, _)| *run == sql).count()
    }
}

fn call(openworks: &Openworks<&TestHost>, path: &str, credential: &str, body: &str) -> Response {
    openworks.handle(&Request {
        path: path.to_owned(),
        credential: credential.to_owned(),
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
}

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

    let committed = host.committed.borrow();
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
    assert!(host.committed.borrow().is_empty());
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
    let once = host.committed.borrow().clone();
    openworks.migrate().unwrap();

    assert_eq!(once.len(), 4);
    assert!(once.iter().all(|(sql, _)| sql.contains("if not exists")));
    assert_eq!(host.committed.borrow()[once.len()..], once[..]);
}
