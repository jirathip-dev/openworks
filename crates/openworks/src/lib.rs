use std::collections::HashMap;

use openworks_core::{Declaration, Fail, Handler, Id, Kind, Mount, Reach, Row, Tx, Value, Who};

mod orgs;

openworks_core::operations! {
    OrgShow => orgs::SHOW,
    OrgRename => orgs::RENAME,
}

impl Operation {
    /// The core's mount: every operation under `/orgs`, reached by the caller's org.
    pub fn mount() -> Mount {
        Self::ALL.iter().fold(Mount::new("/orgs", "orgs"), |mount, operation| {
            mount.operation(operation.declaration(), Reach::Org)
        })
    }
}

/// Files the host stores by key.
pub trait Files {
    fn put(&self, key: &str, bytes: &[u8]) -> Result<(), Fail>;
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, Fail>;
}

/// What openworks needs from the program that embeds it.
pub trait Host {
    fn who(&self, credential: &str) -> Option<Who>;
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
            .chain([openworks_db::AUDIT, openworks_db::EVENT]);
        let unscoped = openworks_db::unscoped(compiled);
        if unscoped.is_empty() { Ok(()) } else { Err(unscoped) }
    }

    /// Answers a request: signs the caller in, checks reach and the needed word, runs the operation.
    pub fn handle(&self, request: &Request) -> Response {
        let Some(route) = self.routes.get(&request.path) else {
            return answer(404, "no such operation");
        };
        if route.reach == Reach::Off {
            return answer(404, "no such operation");
        }
        let Some(who) = self.host.who(&request.credential) else {
            return answer(401, "sign in");
        };
        if !who.holds(route.declaration.needs()) {
            return answer(403, route.declaration.needs().0);
        }
        match self.call(*route, &who, &request.body) {
            Ok(body) => Response { status: 200, body },
            Err(fail) => answer(status(&fail), &fail.to_string()),
        }
    }

    fn call(&self, route: Route, who: &Who, body: &str) -> Result<String, Fail> {
        let declaration = route.declaration;
        let run = match declaration.handler() {
            Handler::Closed(run) => return run(who, body),
            Handler::Store(run) => run,
        };
        let mut answered = None;
        self.host.transact(&mut |tx| {
            let answer = run(
                who,
                &mut Declared {
                    tx: &mut *tx,
                    queries: declaration.queries(),
                },
                body,
            )?;
            if declaration.kind() == Kind::Write {
                self.trail(tx, route, who, body)?;
            }
            answered = Some(answer);
            Ok(())
        })?;
        answered.ok_or_else(|| Fail::Store("the transaction ran no work".to_owned()))
    }

    fn trail(&self, tx: &mut dyn Tx, route: Route, who: &Who, body: &str) -> Result<(), Fail> {
        let operation = format!("{}.{}", route.key, route.declaration.name());
        let org = Value::Text(who.org.0.clone());
        let at = Value::Int(self.host.now());
        tx.run(
            openworks_db::AUDIT,
            &[
                Value::Text(self.host.id().0),
                org.clone(),
                Value::Text(who.person.0.clone()),
                Value::Text(operation.clone()),
                at.clone(),
            ],
        )?;
        tx.run(
            openworks_db::EVENT,
            &[
                Value::Text(self.host.id().0),
                org,
                Value::Text(operation),
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
