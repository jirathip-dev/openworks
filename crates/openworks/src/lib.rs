use std::collections::HashMap;

use openworks_core::{Declaration, Fail, Handler, Id, Kind, Mount, Reach, Role, Row, Tx, Value, Who};

mod members;
mod orgs;

openworks_core::operations! {
    OrgShow => orgs::SHOW,
    OrgRename => orgs::RENAME,
    MemberList => members::LIST,
    MemberSet => members::SET,
    MemberRemove => members::REMOVE,
}

const KEY: &str = "orgs";

impl Operation {
    /// The core's mount: every operation under `/orgs`, reached by the caller's org.
    pub fn mount() -> Mount {
        Self::ALL.iter().fold(Mount::new("/orgs", KEY), |mount, operation| {
            mount.operation(operation.declaration(), Reach::Org)
        })
    }
}

/// Files the host stores by key.
pub trait Files {
    fn put(&self, key: &str, bytes: &[u8]) -> Result<(), Fail>;
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, Fail>;
}

/// The person a credential signs in, by the host's id and email, and the org they act for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Caller {
    pub person: Id,
    pub email: String,
    pub org: Id,
}

/// What openworks needs from the program that embeds it.
pub trait Host {
    fn who(&self, credential: &str) -> Option<Caller>;
    fn transact(&self, work: &mut dyn FnMut(&mut dyn Tx) -> Result<(), Fail>) -> Result<(), Fail>;
    fn id(&self) -> Id;
    fn now(&self) -> i64;
    fn files(&self) -> &dyn Files;
    fn name(&self, key: &str) -> Option<String>;
}

/// A call to an operation, as the host's transport hands it over.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub path: String,
    pub credential: String,
    pub body: String,
}

/// The answer to a request, with an HTTP status code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub body: String,
}

/// Why `build` refused the mounts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refused {
    DuplicatePath(String),
    Unscoped(Vec<&'static str>),
}

#[derive(Clone, Copy)]
struct Route {
    declaration: &'static Declaration,
    reach: Reach,
    key: &'static str,
}

/// Builds an `Openworks` from a host and its mounts.
pub struct Builder<H> {
    host: H,
    mounts: Vec<Mount>,
}

impl<H: Host> Builder<H> {
    /// Adds a mount.
    #[must_use]
    pub fn mount(mut self, mount: Mount) -> Self {
        self.mounts.push(mount);
        self
    }

    /// Routes every mounted operation; refuses a duplicate path or an unscoped query.
    pub fn build(self) -> Result<Openworks<H>, Refused> {
        let mut routes = HashMap::new();
        for mount in &self.mounts {
            for &(declaration, reach) in mount.operations() {
                let path = format!("{}/{}", mount.prefix(), declaration.name());
                let route = Route {
                    declaration,
                    reach,
                    key: mount.key(),
                };
                if routes.insert(path.clone(), route).is_some() {
                    return Err(Refused::DuplicatePath(path));
                }
            }
        }
        let openworks = Openworks {
            host: self.host,
            routes,
        };
        openworks.check().map_err(Refused::Unscoped)?;
        Ok(openworks)
    }
}

/// The router over every mounted operation, on one host.
pub struct Openworks<H> {
    host: H,
    routes: HashMap<String, Route>,
}

impl<H: Host> Openworks<H> {
    /// A builder with the core's mount.
    #[expect(clippy::new_ret_no_self, reason = "the builder is the way in")]
    pub fn new(host: H) -> Builder<H> {
        Builder {
            host,
            mounts: vec![Operation::mount()],
        }
    }

    /// Runs every migration in one transaction.
    pub fn migrate(&self) -> Result<(), Fail> {
        self.host.transact(&mut |tx| openworks_db::migrate(tx))
    }

    /// Every compiled query that touches a `works` table names `org_id`, else the ones that do not.
    pub fn check(&self) -> Result<(), Vec<&'static str>> {
        let compiled = self
            .routes
            .values()
            .flat_map(|route| route.declaration.queries().iter().copied())
            .chain([
                openworks_db::AUDIT,
                openworks_db::EVENT,
                openworks_db::members::FIND,
                openworks_db::members::TIE,
            ]);
        let unscoped = openworks_db::unscoped(compiled);
        if unscoped.is_empty() { Ok(()) } else { Err(unscoped) }
    }

    /// Answers a request: signs the caller in as a member, checks reach and the needed word, runs the operation.
    pub fn handle(&self, request: &Request) -> Response {
        let Some(route) = self.routes.get(&request.path) else {
            return answer(404, "no such operation");
        };
        if route.reach == Reach::Off {
            return answer(404, "no such operation");
        }
        let Some(caller) = self.host.who(&request.credential) else {
            return answer(401, "sign in");
        };
        match self.call(*route, &|tx| self.sign_in(tx, &caller), &request.body) {
            Ok(body) => Response { status: 200, body },
            Err(fail) => answer(status(&fail), &fail.to_string()),
        }
    }

    /// Adds a member to an org by email, or changes their role, on the host's word; `by` is who made the change.
    pub fn set_member(&self, by: Id, org: Id, email: &str, role: Role) -> Result<String, Fail> {
        let who = Who {
            person: by,
            org,
            role: Role::Owner,
        };
        let route = Route {
            declaration: &members::SET,
            reach: Reach::Org,
            key: KEY,
        };
        self.call(route, &|_| Ok(who.clone()), &format!("{email} {}", role.name()))
    }

    fn sign_in(&self, tx: &mut dyn Tx, caller: &Caller) -> Result<Who, Fail> {
        let org = Value::Text(caller.org.0.clone());
        let person = Value::Text(caller.person.0.clone());
        let mut rows = tx.run(openworks_db::members::FIND, &[org.clone(), person.clone()])?;
        let tied = rows.is_empty();
        if tied {
            let email = members::address(&caller.email).map_err(|_| Fail::NotMember)?;
            rows = tx.run(openworks_db::members::TIE, &[org, person, Value::Text(email)])?;
        }
        let role = match rows.first().map(Vec::as_slice) {
            None => return Err(Fail::NotMember),
            Some([Value::Text(role)]) => Role::named(role),
            Some(_) => None,
        };
        let who = Who {
            person: caller.person.clone(),
            org: caller.org.clone(),
            role: role.ok_or_else(|| Fail::Store("a member row holds no role".to_owned()))?,
        };
        if tied {
            self.trail(tx, &who, &format!("{KEY}.join"), &caller.email)?;
        }
        Ok(who)
    }

    fn call(&self, route: Route, sign: &dyn Fn(&mut dyn Tx) -> Result<Who, Fail>, body: &str) -> Result<String, Fail> {
        let declaration = route.declaration;
        let mut answered = None;
        self.host.transact(&mut |tx| {
            let who = sign(tx)?;
            if who.role.grants(declaration.needs()).is_none() {
                return Err(Fail::Needs(declaration.needs()));
            }
            let answer = match declaration.handler() {
                Handler::Closed(run) => run(&who, body)?,
                Handler::Store(run) => run(
                    &who,
                    &mut Declared {
                        tx: &mut *tx,
                        queries: declaration.queries(),
                    },
                    body,
                )?,
            };
            if declaration.kind() == Kind::Write {
                self.trail(tx, &who, &format!("{}.{}", route.key, declaration.name()), body)?;
            }
            answered = Some(answer);
            Ok(())
        })?;
        answered.ok_or_else(|| Fail::Store("the transaction ran no work".to_owned()))
    }

    fn trail(&self, tx: &mut dyn Tx, who: &Who, operation: &str, body: &str) -> Result<(), Fail> {
        let org = Value::Text(who.org.0.clone());
        let at = Value::Int(self.host.now());
        tx.run(
            openworks_db::AUDIT,
            &[
                Value::Text(self.host.id().0),
                org.clone(),
                Value::Text(who.person.0.clone()),
                Value::Text(operation.to_owned()),
                at.clone(),
            ],
        )?;
        tx.run(
            openworks_db::EVENT,
            &[
                Value::Text(self.host.id().0),
                org,
                Value::Text(operation.to_owned()),
                Value::Text(body.to_owned()),
                at,
            ],
        )?;
        Ok(())
    }
}

struct Declared<'a> {
    tx: &'a mut dyn Tx,
    queries: &'static [&'static str],
}

impl Tx for Declared<'_> {
    fn run(&mut self, sql: &'static str, params: &[Value]) -> Result<Vec<Row>, Fail> {
        if !self.queries.contains(&sql) {
            return Err(Fail::Undeclared(sql));
        }
        self.tx.run(sql, params)
    }
}

fn status(fail: &Fail) -> u16 {
    match fail {
        Fail::NotFound => 404,
        Fail::NotMember | Fail::Needs(_) => 403,
        Fail::Invalid(_) => 400,
        Fail::Undeclared(_) | Fail::Store(_) => 500,
    }
}

fn answer(status: u16, body: &str) -> Response {
    Response {
        status,
        body: body.to_owned(),
    }
}
