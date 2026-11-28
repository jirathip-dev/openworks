use std::cell::{Cell, RefCell};

use openworks::{Caller, Files, Host, Openworks, Request, Response, Via};
use openworks_core::{Fail, Id, Mount, Pack, Role, Row, Tx, Value};
use openworks_db::{idempotency, members};

/// The time a test host's clock starts at.
pub const T0: i64 = 1_700_000_000_000;

/// The owner every staffed host signs in.
pub const OWNER: &str = "owner@o.test";

/// A file store that keeps nothing.
pub struct NoFiles;

impl Files for NoFiles {
    fn put(&self, _: &str, _: &[u8]) -> Result<(), Fail> {
        Ok(())
    }
    fn get(&self, _: &str) -> Result<Option<Vec<u8>>, Fail> {
        Ok(None)
    }
}

/// An area's rows that answer every query with none.
#[derive(Clone, Default)]
pub struct NoRows;

impl Tx for NoRows {
    fn run(&mut self, _: &'static str, _: &[Value]) -> Result<Vec<Row>, Fail> {
        Ok(Vec::new())
    }
}

/// A member as the store keeps them: their email, the person tied to it and their role.
#[derive(Clone, Debug, PartialEq)]
pub struct Member {
    pub email: String,
    pub person: Option<String>,
    pub role: String,
}

/// A host whose credential is an email, `assistant:` before it for an assistant, signing in to org `o1`.
#[derive(Default)]
pub struct TestHost<A = NoRows> {
    pub committed: RefCell<Store<A>>,
    pub transactions: RefCell<usize>,
    pub next_id: RefCell<u32>,
    pub clock: Cell<i64>,
}

/// Every statement run, the idempotency keys and members, and an area's own rows for the rest.
#[derive(Clone, Default)]
pub struct Store<A> {
    pub statements: Vec<(&'static str, Vec<Value>)>,
    pub keys: Vec<Row>,
    pub members: Vec<Member>,
    pub area: A,
}

fn text(value: &Value) -> String {
    match value {
        Value::Text(text) => text.clone(),
        _ => String::new(),
    }
}

impl<A: Tx> Tx for Store<A> {
    fn run(&mut self, sql: &'static str, params: &[Value]) -> Result<Vec<Row>, Fail> {
        self.statements.push((sql, params.to_vec()));
        Ok(match sql {
            openworks_db::orgs::LIST => vec![vec![
                Value::Text("o1".to_owned()),
                Value::Text("First \"org\"".to_owned()),
            ]],
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
            _ => match self.member_rows(sql, params) {
                Some(rows) => rows,
                None => self.area.run(sql, params)?,
            },
        })
    }
}

impl<A> Store<A> {
    fn member_rows(&mut self, sql: &'static str, params: &[Value]) -> Option<Vec<Row>> {
        let param = |at: usize| params.get(at).map_or_else(String::new, text);
        let role = |member: &Member| vec![Value::Text(member.role.clone())];
        Some(match sql {
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
            _ => return None,
        })
    }
}

impl<A: Tx + Clone> Host for &TestHost<A> {
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

impl<A> TestHost<A> {
    /// How many times the committed statements ran a query.
    pub fn count(&self, sql: &str) -> usize {
        self.committed
            .borrow()
            .statements
            .iter()
            .filter(|(run, _)| *run == sql)
            .count()
    }
}

/// The router with a member of each role and the mount, its statements and transactions cleared.
pub fn staffed<A: Tx + Clone>(host: &TestHost<A>, mount: Option<Mount>) -> Openworks<&TestHost<A>> {
    staffed_with(host, mount, Pack::default())
}

/// The router with a member of each role, the mount and the pack, its statements and transactions cleared.
pub fn staffed_with<A: Tx + Clone>(host: &TestHost<A>, mount: Option<Mount>, pack: Pack) -> Openworks<&TestHost<A>> {
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
    host.committed.borrow_mut().statements.clear();
    *host.transactions.borrow_mut() = 0;
    openworks
}

/// Sends a body to a path as a credential, without an idempotency key.
pub fn call<A: Tx + Clone>(openworks: &Openworks<&TestHost<A>>, path: &str, credential: &str, body: &str) -> Response {
    send(openworks, path, credential, None, body)
}

/// Sends a body to a path as a credential, under the key if one is given.
pub fn send<A: Tx + Clone>(
    openworks: &Openworks<&TestHost<A>>,
    path: &str,
    credential: &str,
    key: Option<&str>,
    body: &str,
) -> Response {
    openworks.handle(&Request {
        path: path.to_owned(),
        credential: credential.to_owned(),
        idempotency_key: key.map(str::to_owned),
        body: body.to_owned(),
    })
}
