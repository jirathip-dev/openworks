use std::collections::HashMap;

use openworks_core::{Call, Change, Declaration, Fail, Handler, Hook, Id, Kind, Mount, Reach, Row, Tx, Value, Who};

pub use openworks_core::Files;
pub use series::{Period, Series};

mod due;
mod files;
mod orgs;
mod series;

const AFTER_COMMIT_ATTEMPTS: usize = 3;

openworks_core::operations! {
    OrgShow => orgs::SHOW,
    OrgRename => orgs::RENAME,
    FileAttach => files::ATTACH,
    FileList => files::LIST,
}

const HOMES: [(&str, &str); 2] = [("/orgs", "orgs"), ("/files", "files")];

impl Operation {
    /// The path prefix and extension key the operation is mounted under.
    pub const fn home(self) -> (&'static str, &'static str) {
        match self {
            Self::OrgShow | Self::OrgRename => ("/orgs", "orgs"),
            Self::FileAttach | Self::FileList => ("/files", "files"),
        }
    }

    /// The operation's path.
    pub fn path(self) -> String {
        format!("{}/{}", self.home().0, self.declaration().name())
    }

    /// The core's mounts: every operation under its prefix, reached by the caller's org.
    pub fn mounts() -> Vec<Mount> {
        HOMES
            .iter()
            .map(|&(prefix, key)| {
                Self::ALL
                    .iter()
                    .filter(|operation| operation.home().0 == prefix)
                    .fold(Mount::new(prefix, key), |mount, operation| {
                        mount.operation(operation.declaration(), Reach::Org)
                    })
            })
            .collect()
    }
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
    pub idempotency_key: Option<String>,
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
    hooks: Vec<Box<dyn Hook>>,
}

impl<H: Host> Builder<H> {
    /// Adds a mount.
    #[must_use]
    pub fn mount(mut self, mount: Mount) -> Self {
        self.mounts.push(mount);
        self
    }

    /// Adds a hook, run after the ones already added.
    #[must_use]
    pub fn hook(mut self, hook: impl Hook + 'static) -> Self {
        self.hooks.push(Box::new(hook));
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
            hooks: self.hooks,
        };
        openworks.check().map_err(Refused::Unscoped)?;
        Ok(openworks)
    }
}

/// The router over every mounted operation, on one host.
pub struct Openworks<H> {
    host: H,
    routes: HashMap<String, Route>,
    hooks: Vec<Box<dyn Hook>>,
}

impl<H: Host> Openworks<H> {
    /// A builder with the core's mounts.
    #[expect(clippy::new_ret_no_self, reason = "the builder is the way in")]
    pub fn new(host: H) -> Builder<H> {
        Builder {
            host,
            mounts: Operation::mounts(),
            hooks: Vec::new(),
        }
    }

    /// Runs every migration in one transaction.
    pub fn migrate(&self) -> Result<(), Fail> {
        self.host.transact(&mut |tx| openworks_db::migrate(tx))
    }

    /// Runs every due job for every org, each job in one transaction; running it twice does what once did.
    pub fn run_due(&self, now: i64) -> Result<(), Fail> {
        for job in due::JOBS {
            self.host.transact(&mut |tx| {
                for row in tx.run(openworks_db::orgs::ALL, &[])? {
                    if let Some(Value::Text(org)) = row.first() {
                        let mut declared = Declared {
                            tx: &mut *tx,
                            queries: job.queries,
                        };
                        (job.run)(&Id(org.clone()), &mut declared, now)?;
                    }
                }
                Ok(())
            })?;
        }
        Ok(())
    }

    /// Every compiled query that touches a `works` table names `org_id`, else the ones that do not.
    pub fn check(&self) -> Result<(), Vec<&'static str>> {
        let compiled = self
            .routes
            .values()
            .flat_map(|route| route.declaration.queries().iter().copied())
            .chain(due::JOBS.iter().flat_map(|job| job.queries.iter().copied()))
            .chain([
                openworks_db::AUDIT,
                openworks_db::EVENT,
                openworks_db::orgs::ALL,
                openworks_db::idempotency::FIND,
                openworks_db::idempotency::SAVE,
                openworks_db::series::NEXT,
            ]);
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
        match self.call(*route, &who, request) {
            Ok(body) => Response { status: 200, body },
            Err(fail) => answer(status(&fail), &fail.to_string()),
        }
    }

    fn call(&self, route: Route, who: &Who, request: &Request) -> Result<String, Fail> {
        let declaration = route.declaration;
        let run = match declaration.handler() {
            Handler::Closed(run) => return run(who, &request.body),
            Handler::Store(run) => run,
        };
        let key = match (declaration.kind(), request.idempotency_key.as_deref()) {
            (Kind::Create, None | Some("")) => {
                return Err(Fail::Invalid("a create needs an Idempotency-Key".to_owned()));
            }
            (Kind::Create, key) => key,
            _ => None,
        };
        let change = Change {
            org: who.org.clone(),
            person: who.person.clone(),
            operation: format!("{}.{}", route.key, declaration.name()),
            body: request.body.clone(),
            at: self.host.now(),
        };
        let mut answered = None;
        let mut changed = false;
        self.host.transact(&mut |tx| {
            changed = false;
            answered = key.map(|key| replay(tx, &change, key)).transpose()?.flatten();
            if answered.is_some() {
                return Ok(());
            }
            let call = Call {
                who,
                body: &request.body,
                at: change.at,
                id: self.host.id(),
                files: self.host.files(),
            };
            let answer = run(
                &call,
                &mut Declared {
                    tx: &mut *tx,
                    queries: declaration.queries(),
                },
            )?;
            if declaration.kind() != Kind::Read {
                self.trail(tx, &change)?;
                if let Some(key) = key {
                    keep(tx, &change, key, &answer)?;
                }
                for hook in &self.hooks {
                    hook.before_commit(&change)?;
                }
                changed = true;
            }
            answered = Some(answer);
            Ok(())
        })?;
        if changed {
            self.after_commit(&change);
        }
        answered.ok_or_else(|| Fail::Store("the transaction ran no work".to_owned()))
    }

    fn after_commit(&self, change: &Change) {
        for hook in &self.hooks {
            for _ in 0..AFTER_COMMIT_ATTEMPTS {
                if hook.after_commit(change).is_ok() {
                    break;
                }
            }
        }
    }

    fn trail(&self, tx: &mut dyn Tx, change: &Change) -> Result<(), Fail> {
        let org = Value::Text(change.org.0.clone());
        let operation = Value::Text(change.operation.clone());
        let at = Value::Int(change.at);
        tx.run(
            openworks_db::AUDIT,
            &[
                Value::Text(self.host.id().0),
                org.clone(),
                Value::Text(change.person.0.clone()),
                operation.clone(),
                at.clone(),
            ],
        )?;
        tx.run(
            openworks_db::EVENT,
            &[
                Value::Text(self.host.id().0),
                org,
                operation,
                Value::Text(change.body.clone()),
                at,
            ],
        )?;
        Ok(())
    }
}

fn replay(tx: &mut dyn Tx, change: &Change, key: &str) -> Result<Option<String>, Fail> {
    let rows = tx.run(
        openworks_db::idempotency::FIND,
        &[
            Value::Text(change.org.0.clone()),
            Value::Text(change.operation.clone()),
            Value::Text(key.to_owned()),
        ],
    )?;
    Ok(match rows.first().and_then(|row| row.first()) {
        Some(Value::Text(answer)) => Some(answer.clone()),
        _ => None,
    })
}

fn keep(tx: &mut dyn Tx, change: &Change, key: &str, answer: &str) -> Result<(), Fail> {
    tx.run(
        openworks_db::idempotency::SAVE,
        &[
            Value::Text(change.org.0.clone()),
            Value::Text(change.operation.clone()),
            Value::Text(key.to_owned()),
            Value::Text(answer.to_owned()),
            Value::Int(change.at),
        ],
    )?;
    Ok(())
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
