mod common;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use common::NoFiles;
use openworks::{Caller, Files, Host, Openworks, Operation, Refused, Request, Response, Via};
use openworks_core::{
    Approval, Change, Declaration, Fail, Hook, Id, Kind, Mount, Pack, Reach, Role, Row, Scope, Tx, Value, Word,
};
use openworks_db::{approvals, attachments, clients, idempotency, members, sites};

type Statement = (&'static str, Vec<Value>);

const T0: i64 = 1_700_000_000_000;
const DAY: i64 = 24 * 60 * 60 * 1000;

#[derive(Clone, Debug, PartialEq)]
struct Member {
    email: String,
    person: Option<String>,
    role: String,
}

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
    members: Vec<Member>,
    kinds: Vec<Row>,
    approvals: Vec<Row>,
    files: Vec<Row>,
}

fn text(value: &Value) -> String {
    match value {
        Value::Text(text) => text.clone(),
        _ => String::new(),
    }
}

impl Tx for Store {
    fn run(&mut self, sql: &'static str, params: &[Value]) -> Result<Vec<Row>, Fail> {
        self.statements.push((sql, params.to_vec()));
        Ok(match sql {
            openworks_db::orgs::LIST => vec![vec![
                Value::Text("o1".to_owned()),
                Value::Text("First \"org\"".to_owned()),
            ]],
            openworks_db::orgs::ALL => vec![vec![Value::Text("o1".to_owned())]],
            clients::GET => vec![vec![
                Value::Text("c1".to_owned()),
                Value::Text("Acme".to_owned()),
                Value::Null,
                Value::Null,
            ]],
            sites::GET => vec![
                ["s1", "c1", "Yard", "", ""]
                    .map(|field| Value::Text(field.to_owned()))
                    .into_iter()
                    .chain([Value::Null])
                    .collect(),
            ],
            clients::DEACTIVATE | clients::SYNC | sites::DEACTIVATE => vec![vec![Value::Text("c1".to_owned())]],
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
            _ => match self.approval_rows(sql, params) {
                Some(rows) => rows,
                None => self.member_rows(sql, params),
            },
        })
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

impl Store {
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

    fn member_rows(&mut self, sql: &'static str, params: &[Value]) -> Vec<Row> {
        let param = |at: usize| params.get(at).map_or_else(String::new, text);
        let role = |member: &Member| vec![Value::Text(member.role.clone())];
        match sql {
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
        }
    }
}

impl Host for &TestHost {
    fn who(&self, credential: &str) -> Option<Caller> {
        let (via, email) = match credential.strip_prefix("assistant:") {
            Some(email) => (Via::Assistant, email),
            None => (Via::Person, credential),
        };
        email.contains('@').then(|| Caller {
            person: Id(format!("person:{email}")),
            email: email.to_owned(),
            org: Id("o1".to_owned()),
            via,
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

const OWNER: &str = "owner@o.test";

fn staffed(host: &TestHost, mount: Option<Mount>) -> Openworks<&TestHost> {
    staffed_with(host, mount, Pack::default())
}

fn staffed_with(host: &TestHost, mount: Option<Mount>, pack: Pack) -> Openworks<&TestHost> {
    let builder = Openworks::new(host).pack(pack);
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
        assert_eq!(call(&openworks, "/orgs/list", email, "").status, 200);
    }
    let mut store = host.committed.borrow_mut();
    store.statements.clear();
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
    drop(store);
    *host.transactions.borrow_mut() = 0;
    openworks
}

fn email(role: Role) -> String {
    format!("{}@o.test", role.name())
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

const ANY_BODY: &str = "job-1\nplan.txt\nfirst floor";

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

const WHOAMI: Declaration = openworks_core::closed!("whoami", needs: Read, |who, _| Ok(who.person.0.clone()));

fn notes() -> Mount {
    Mount::new("/notes", "notes")
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

fn body(operation: Operation) -> &'static str {
    match operation {
        Operation::MemberSet => "new@o.test viewer",
        Operation::MemberRemove => "viewer@o.test",
        Operation::OrgList | Operation::OrgRename | Operation::MemberList => "New name",
        Operation::FileAttach | Operation::FileList => ANY_BODY,
        Operation::ApprovalSaveKind => "inspection signature",
        Operation::ApprovalRequest => "visit job-3\nplease",
        Operation::ApprovalDecide => "a1 yes",
        Operation::ApprovalReset => "a2",
        Operation::ApprovalPending | Operation::ApprovalRecent => "",
        Operation::ClientSave => "\nAcme\n",
        Operation::ClientSync => "ref-1\nAcme",
        Operation::SiteSave => "\nc1\nYard\n1 Main Road\n",
        Operation::ClientList
        | Operation::ClientGet
        | Operation::ClientDeactivate
        | Operation::SiteList
        | Operation::SiteGet
        | Operation::SiteDeactivate => "c1",
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
        let path = operation.path();
        for role in Role::ALL {
            let host = TestHost::default();
            let openworks = staffed(&host, None);

            let answered = send(&openworks, &path, &email(role), Some("k"), body(*operation));

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
                assert!(
                    host.committed.borrow().statements.is_empty(),
                    "{path} as {}",
                    role.name()
                );
            }
        }
    }
}

#[test]
fn a_test_host_mounts_the_router_and_answers() {
    let host = TestHost::default();
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
    let host = TestHost::default();
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
fn a_member_is_added_by_email_and_tied_at_first_sign_in() {
    let host = TestHost::default();
    let openworks = staffed(&host, None);

    let added = call(&openworks, "/orgs/set-member", OWNER, " New@O.test  manager");
    assert_eq!(added.body, "new@o.test manager");
    assert_eq!(
        host.committed
            .borrow()
            .members
            .last()
            .and_then(|member| member.person.clone()),
        None
    );

    assert_eq!(call(&openworks, "/orgs/rename", "new@o.test", "x").status, 403);
    assert_eq!(call(&openworks, "/orgs/members", "new@o.test", "").status, 200);
    assert_eq!(call(&openworks, "/orgs/members", "new@o.test", "").status, 200);

    let tied = host
        .committed
        .borrow()
        .members
        .last()
        .and_then(|member| member.person.clone());
    assert_eq!(tied.as_deref(), Some("person:new@o.test"));
    assert_eq!(host.count(members::TIE), 1);
    let joined = Value::Text("orgs.join".to_owned());
    let committed = host.committed.borrow();
    let audit = committed
        .statements
        .iter()
        .filter(|(sql, params)| *sql == openworks_db::AUDIT && params.get(3) == Some(&joined));
    let event = committed
        .statements
        .iter()
        .filter(|(sql, params)| *sql == openworks_db::EVENT && params.get(2) == Some(&joined));
    assert_eq!((audit.count(), event.count()), (1, 1));
}

#[test]
fn a_person_who_is_no_member_is_refused() {
    let host = TestHost::default();
    let openworks = staffed(&host, None);

    let refused = call(&openworks, "/orgs/list", "stranger@o.test", "");

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
        .committed
        .borrow()
        .members
        .iter()
        .find(|member| member.email == OWNER)
        .map(|member| member.role.clone());
    assert_eq!(owner.as_deref(), Some("owner"));

    let empty = TestHost::default();
    let fresh = Openworks::new(&empty).build().unwrap();
    assert!(fresh.set_member(by(), org(), "first@o.test", Role::Viewer).is_err());
    assert_eq!(empty.committed.borrow().members.len(), 0);

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
        let path = operation.path();

        assert_eq!(
            send(&openworks, &path, OWNER, Some("k"), body(*operation)).status,
            200,
            "{path}"
        );

        let trail = usize::from(declaration.kind() != Kind::Read);
        assert_eq!(host.count(openworks_db::AUDIT), trail, "{path}");
        assert_eq!(host.count(openworks_db::EVENT), trail, "{path}");
    }
}

#[test]
fn a_write_names_its_mount_key_and_operation_in_the_trail() {
    let host = TestHost::default();
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
    let host = TestHost::default();
    let openworks = staffed(&host, Some(notes()));

    assert_eq!(call(&openworks, "/orgs/rename", OWNER, "  ").status, 400);
    assert_eq!(call(&openworks, "/notes/stray", OWNER, "").status, 500);
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

    let refused = Openworks::new(&host).mount(Operation::mounts().remove(0)).build().err();

    assert_eq!(refused, Some(Refused::DuplicatePath("/orgs/list".to_owned())));
}

#[test]
fn migrate_runs_every_statement_and_again_does_the_same() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();

    openworks.migrate().unwrap();
    let once = host.committed.borrow().statements.clone();
    openworks.migrate().unwrap();

    assert_eq!(once.len(), 14);
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

fn owned() -> TestHost {
    let host = TestHost::default();
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
    let host = TestHost::default();
    let openworks = staffed(&host, Some(notes()));

    for key in [None, Some("")] {
        let refused = send(&openworks, "/notes/open", OWNER, key, "hello");
        assert_eq!(refused.status, 400);
    }
    assert_eq!(host.committed.borrow().statements.len(), 0);
}

#[test]
fn a_create_repeated_under_its_key_runs_once_and_answers_the_same() {
    let host = TestHost::default();
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
    let host = TestHost::default();
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
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();

    let document = openworks.openapi();

    for operation in Operation::ALL {
        let declaration = operation.declaration();
        let path = format!(
            r#""{}":{{"post":{{"x-needs":"{}","#,
            operation.path(),
            declaration.needs().name()
        );
        assert!(document.contains(&path), "{document}");
    }
    let orgs =
        r#""orgs":{"type":"object","properties":{"id":{"type":"string"},"ext":{"type":"object","properties":{}}}}"#;
    assert!(document.contains(orgs), "{document}");
}

const MANAGER: &str = "manager@o.test";

fn ask(openworks: &Openworks<&TestHost>, key: &str, body: &str) -> Response {
    send(openworks, "/approvals/request", "member@o.test", Some(key), body)
}

fn decide(openworks: &Openworks<&TestHost>, who: &str, body: &str) -> String {
    call(openworks, "/approvals/decide", who, body).body
}

fn listed(openworks: &Openworks<&TestHost>, read: &str) -> Vec<String> {
    let body = call(openworks, &format!("/approvals/{read}"), "viewer@o.test", "").body;
    body.lines().map(str::to_owned).collect()
}

#[test]
fn a_rejected_approval_is_asked_for_again() {
    let host = TestHost::default();
    let openworks = staffed(&host, None);

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
    let host = TestHost::default();
    let openworks = staffed(&host, None);

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
    let host = TestHost::default();
    let openworks = staffed(&host, None);
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
    let host = TestHost::default();
    let openworks = staffed(&host, None);

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
    let host = TestHost::default();
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
    let host = TestHost::default();
    let openworks = staffed(&host, None);

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
