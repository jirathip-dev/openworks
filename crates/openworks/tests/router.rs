use std::cell::RefCell;

use openworks::{Caller, Files, Host, Openworks, Operation, Refused, Request, Response};
use openworks_core::{Declaration, Fail, Id, Kind, Mount, Reach, Role, Row, Scope, Tx, Value, Word};
use openworks_db::members;

type Statement = (&'static str, Vec<Value>);

#[derive(Clone, Debug, PartialEq)]
struct Member {
    email: String,
    person: Option<String>,
    role: String,
}

#[derive(Default)]
struct TestHost {
    committed: RefCell<Vec<Statement>>,
    members: RefCell<Vec<Member>>,
    transactions: RefCell<usize>,
    next_id: RefCell<u32>,
}

struct Log<'a> {
    statements: &'a mut Vec<Statement>,
    members: &'a mut Vec<Member>,
}

fn text(value: &Value) -> String {
    match value {
        Value::Text(text) => text.clone(),
        _ => String::new(),
    }
}

impl Tx for Log<'_> {
    fn run(&mut self, sql: &'static str, params: &[Value]) -> Result<Vec<Row>, Fail> {
        self.statements.push((sql, params.to_vec()));
        let param = |at: usize| params.get(at).map_or_else(String::new, text);
        let role = |member: &Member| vec![Value::Text(member.role.clone())];
        let rows = match sql {
            openworks_db::orgs::SHOW => vec![vec![Value::Text("First org".to_owned())]],
            members::FIND => self
                .members
                .iter()
                .filter(|member| member.person == Some(param(1)))
                .map(role)
                .collect(),
            members::TIE => self
                .members
                .iter_mut()
                .filter(|member| member.email == param(2) && member.person.is_none())
                .map(|member| {
                    member.person = Some(param(1));
                    role(member)
                })
                .collect(),
            members::LIST => self
                .members
                .iter()
                .map(|member| vec![Value::Text(member.email.clone()), Value::Text(member.role.clone())])
                .collect(),
            members::HOLDING => self
                .members
                .iter()
                .filter(|member| member.role == param(1))
                .map(|member| vec![Value::Text(member.email.clone())])
                .collect(),
            members::SET => {
                match self.members.iter_mut().find(|member| member.email == param(1)) {
                    Some(member) => member.role = param(2),
                    None => self.members.push(Member {
                        email: param(1),
                        person: None,
                        role: param(2),
                    }),
                }
                Vec::new()
            }
            members::REMOVE => {
                self.members.retain(|member| member.email != param(1));
                Vec::new()
            }
            _ => Vec::new(),
        };
        Ok(rows)
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
    fn who(&self, credential: &str) -> Option<Caller> {
        credential.contains('@').then(|| Caller {
            person: Id(format!("person:{credential}")),
            email: credential.to_owned(),
            org: Id("o1".to_owned()),
        })
    }

    fn transact(&self, work: &mut dyn FnMut(&mut dyn Tx) -> Result<(), Fail>) -> Result<(), Fail> {
        *self.transactions.borrow_mut() += 1;
        let mut statements = Vec::new();
        let mut members = self.members.borrow().clone();
        work(&mut Log {
            statements: &mut statements,
            members: &mut members,
        })?;
        self.committed.borrow_mut().extend(statements);
        *self.members.borrow_mut() = members;
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

const OWNER: &str = "owner@o.test";

fn staffed(host: &TestHost, mount: Option<Mount>) -> Openworks<&TestHost> {
    let builder = Openworks::new(host);
    let openworks = match mount {
        Some(mount) => builder.mount(mount),
        None => builder,
    }
    .build()
    .unwrap();
    for (email, role) in [
        (OWNER, Role::Owner),
        ("manager@o.test", Role::Manager),
        ("member@o.test", Role::Member),
        ("viewer@o.test", Role::Viewer),
    ] {
        openworks
            .set_member(Id("host".to_owned()), Id("o1".to_owned()), email, role)
            .unwrap();
        assert_eq!(call(&openworks, "/orgs/show", email, "").status, 200);
    }
    host.committed.borrow_mut().clear();
    *host.transactions.borrow_mut() = 0;
    openworks
}

fn email(role: Role) -> String {
    format!("{}@o.test", role.name())
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

const ADD_NOTE: Declaration = openworks_core::write!("add", needs: Write, queries: [NOTE], |who, tx, body| {
    tx.run(NOTE, &[Value::Text(who.org.0.clone()), Value::Text(body.to_owned())])?;
    Ok(String::new())
});

const STRAY_NOTE: Declaration = openworks_core::write!("stray", needs: Write, queries: [NOTE], |who, tx, _| {
    tx.run(STRAY, &[Value::Text(who.org.0.clone())])?;
    Ok(String::new())
});

const UNSCOPED: Declaration = openworks_core::read!(
    "all",
    needs: Read,
    queries: ["select body from works.notes"],
    |_, _, _| Ok(String::new())
);

const WHOAMI: Declaration = openworks_core::closed!("whoami", needs: Read, |who, _| Ok(who.person.0.clone()));

fn notes() -> Mount {
    Mount::new("/notes", "notes")
        .operation(&ADD_NOTE, Reach::Org)
        .operation(&STRAY_NOTE, Reach::Org)
        .operation(&WHOAMI, Reach::Org)
        .operation(&SWITCHED_OFF, Reach::Off)
}

const SWITCHED_OFF: Declaration = openworks_core::closed!("off", needs: Read, |_, _| Ok(String::new()));

fn body(operation: Operation) -> &'static str {
    match operation {
        Operation::MemberSet => "new@o.test viewer",
        Operation::MemberRemove => "viewer@o.test",
        Operation::OrgShow | Operation::OrgRename | Operation::MemberList => "New name",
    }
}

#[test]
fn the_role_table_grants_every_word() {
    use Scope::{All, Own};
    let table = [
        (Role::Owner, [Some(All), Some(All), Some(All), Some(All), Some(All)]),
        (Role::Manager, [Some(All), Some(All), Some(All), None, None]),
        (Role::Member, [Some(All), Some(Own), None, None, None]),
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
    for operation in Operation::ALL {
        let declaration = operation.declaration();
        let path = format!("/orgs/{}", declaration.name());
        for role in Role::ALL {
            let host = TestHost::default();
            let openworks = staffed(&host, None);

            let answered = call(&openworks, &path, &email(role), body(*operation));

            let expected = match role.grants(declaration.needs()) {
                Some(_) => 200,
                None => 403,
            };
            assert_eq!(
                answered.status,
                expected,
                "{path} as {}: {}",
                role.name(),
                answered.body
            );
            if expected == 403 {
                assert_eq!(answered.body, format!("needs {}", declaration.needs().name()));
                assert!(host.committed.borrow().is_empty(), "{path} as {}", role.name());
            }
        }
    }
}

#[test]
fn a_test_host_mounts_the_router_and_answers() {
    let host = TestHost::default();
    let openworks = staffed(&host, Some(notes()));

    let shown = call(&openworks, "/orgs/show", "viewer@o.test", "");
    assert_eq!(
        shown,
        Response {
            status: 200,
            body: "First org".to_owned()
        }
    );
    assert_eq!(
        call(&openworks, "/notes/whoami", "viewer@o.test", "").body,
        "person:viewer@o.test"
    );
    assert_eq!(call(&openworks, "/orgs/missing", OWNER, "").status, 404);
    assert_eq!(call(&openworks, "/notes/off", OWNER, "").status, 404);
    assert_eq!(call(&openworks, "/orgs/show", "stranger", "").status, 401);
}

#[test]
fn a_closed_operation_runs_no_query_but_the_sign_in() {
    let host = TestHost::default();
    let openworks = staffed(&host, Some(notes()));

    assert_eq!(call(&openworks, "/notes/whoami", OWNER, "").status, 200);
    assert_eq!(*host.transactions.borrow(), 1);
    let committed = host.committed.borrow();
    assert!(committed.iter().all(|(sql, _)| *sql == members::FIND), "{committed:?}");
}

#[test]
fn a_member_is_added_by_email_and_tied_at_first_sign_in() {
    let host = TestHost::default();
    let openworks = staffed(&host, None);

    let added = call(&openworks, "/orgs/set-member", OWNER, " New@O.test  manager");
    assert_eq!(added.body, "new@o.test manager");
    assert_eq!(
        host.members.borrow().last().and_then(|member| member.person.clone()),
        None
    );

    assert_eq!(call(&openworks, "/orgs/rename", "new@o.test", "x").status, 403);
    assert_eq!(call(&openworks, "/orgs/members", "new@o.test", "").status, 200);
    assert_eq!(call(&openworks, "/orgs/members", "new@o.test", "").status, 200);

    let tied = host.members.borrow().last().and_then(|member| member.person.clone());
    assert_eq!(tied.as_deref(), Some("person:new@o.test"));
    assert_eq!(host.count(members::TIE), 1);
    let joined = Value::Text("orgs.join".to_owned());
    let committed = host.committed.borrow();
    let audit = committed
        .iter()
        .filter(|(sql, params)| *sql == openworks_db::AUDIT && params.get(3) == Some(&joined));
    let event = committed
        .iter()
        .filter(|(sql, params)| *sql == openworks_db::EVENT && params.get(2) == Some(&joined));
    assert_eq!((audit.count(), event.count()), (1, 1));
}

#[test]
fn a_person_who_is_no_member_is_refused() {
    let host = TestHost::default();
    let openworks = staffed(&host, None);

    let refused = call(&openworks, "/orgs/show", "stranger@o.test", "");

    assert_eq!(
        refused,
        Response {
            status: 403,
            body: "not a member".to_owned()
        }
    );
}

#[test]
fn a_company_always_has_an_owner() {
    let host = TestHost::default();
    let openworks = staffed(&host, None);
    let org = || Id("o1".to_owned());
    let by = || Id("host".to_owned());

    assert_eq!(call(&openworks, "/orgs/remove-member", OWNER, OWNER).status, 400);
    assert_eq!(
        call(&openworks, "/orgs/set-member", OWNER, "owner@o.test viewer").status,
        400
    );
    let lone = openworks.set_member(by(), org(), OWNER, Role::Manager);
    assert_eq!(lone, Err(Fail::Invalid("a company always has an Owner".to_owned())));
    let owner = host
        .members
        .borrow()
        .iter()
        .find(|member| member.email == OWNER)
        .map(|member| member.role.clone());
    assert_eq!(owner.as_deref(), Some("owner"));

    let empty = TestHost::default();
    let fresh = Openworks::new(&empty).build().unwrap();
    assert!(fresh.set_member(by(), org(), "first@o.test", Role::Viewer).is_err());
    assert!(empty.members.borrow().is_empty());

    assert_eq!(
        call(&openworks, "/orgs/set-member", OWNER, "manager@o.test owner").status,
        200
    );
    assert_eq!(
        call(&openworks, "/orgs/remove-member", "manager@o.test", OWNER).status,
        200
    );
    assert_eq!(
        call(&openworks, "/orgs/members", "manager@o.test", "").body,
        "manager@o.test owner\nmember@o.test member\nviewer@o.test viewer"
    );
}

#[test]
fn a_host_sets_members_through_the_api() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();

    let set = openworks.set_member(Id("host".to_owned()), Id("o1".to_owned()), "boss@o.test", Role::Owner);

    assert_eq!(set, Ok("boss@o.test owner".to_owned()));
    assert_eq!(call(&openworks, "/orgs/rename", "boss@o.test", "Renamed").status, 200);
    assert_eq!(host.count(openworks_db::AUDIT), 3);
    assert_eq!(host.count(openworks_db::EVENT), 3);
}

#[test]
fn every_write_leaves_one_audit_row_and_one_event() {
    for operation in Operation::ALL {
        let host = TestHost::default();
        let openworks = staffed(&host, None);
        let declaration = operation.declaration();
        let path = format!("/orgs/{}", declaration.name());

        assert_eq!(call(&openworks, &path, OWNER, body(*operation)).status, 200, "{path}");

        let trail = usize::from(declaration.kind() == Kind::Write);
        assert_eq!(host.count(openworks_db::AUDIT), trail, "{path}");
        assert_eq!(host.count(openworks_db::EVENT), trail, "{path}");
    }
}

#[test]
fn a_write_names_its_mount_key_and_operation_in_the_trail() {
    let host = TestHost::default();
    let openworks = staffed(&host, Some(notes()));

    assert_eq!(call(&openworks, "/notes/add", "member@o.test", "hello").status, 200);

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
    let openworks = staffed(&host, Some(notes()));

    assert_eq!(call(&openworks, "/orgs/rename", OWNER, "  ").status, 400);
    assert_eq!(call(&openworks, "/notes/stray", OWNER, "").status, 500);
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

    assert_eq!(once.len(), 5);
    assert!(once.iter().all(|(sql, _)| sql.contains("if not exists")));
    assert_eq!(host.committed.borrow()[once.len()..], once[..]);
}
