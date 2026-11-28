use std::io::Write as _;
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use openworks::{Caller, Host, Offer, Openworks, Refused, Request, Via};
use openworks_core::{Fail, Id, Role, Tx, Value};
use serde_json::{Value as Json, json};

use crate::http::{self, Answer, Asked};
use crate::issuer::Issuer;
use crate::routes::{Gate, Own};
use crate::store::Store;
use crate::{tokens, web};

const READ_TIMEOUT: Duration = Duration::from_secs(10);

const MEMBER: &str =
    "select role from works.members where org_id = $1 and (person_id = $2 or (person_id is null and email = $3))";

const ORG: &str = "insert into works.orgs (org_id, name, created_at) values ($1, $2, $3)";

const PROPOSE: &str = "insert into server.proposals (proposal_id, org_id, person_id, path, body, state, at) values ($1, $2, $3, $4, $5, 'open', $6)";

const OPEN: &str = "select proposal_id, path, body, at from server.proposals where org_id = $1 and person_id = $2 and state = 'open' order by at, proposal_id";

const CLOSE: &str = "update server.proposals set state = $4 where proposal_id = $1 and org_id = $2 and person_id = $3 and state = 'open' returning path, body";

const REOPEN: &str = "update server.proposals set state = 'open' where proposal_id = $1 and org_id = $2";

/// The standalone server: the router over the store, the issuer people sign in with and the web bundle.
pub struct Server<'a> {
    store: &'a Store,
    openworks: Openworks<&'a Store>,
    issuer: Issuer,
    web: PathBuf,
}

impl<'a> Server<'a> {
    /// Routes the core's operations over the store.
    pub fn new(store: &'a Store, issuer: Issuer, web: PathBuf) -> Result<Self, Refused> {
        Ok(Self {
            store,
            openworks: Openworks::new(store).build()?,
            issuer,
            web,
        })
    }

    /// Answers requests on `listener` one at a time, running due work every `every`.
    pub fn serve(&self, listener: TcpListener, every: Duration) {
        let (send, streams) = mpsc::channel::<TcpStream>();
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                if send.send(stream).is_err() {
                    break;
                }
            }
        });
        let mut due = Instant::now();
        loop {
            match streams.recv_timeout(due.saturating_duration_since(Instant::now())) {
                Ok(stream) => self.reply(&stream),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
            if Instant::now() >= due {
                if let Err(fail) = self.openworks.run_due(self.store.now()) {
                    log(&format!("due work: {fail}"));
                }
                due = Instant::now() + every;
            }
        }
    }

    /// Answers one request.
    pub fn answer(&self, asked: &Asked) -> Answer {
        let body = String::from_utf8_lossy(&asked.body);
        let Some(own) = Own::find(&asked.method, &asked.path) else {
            return match (asked.method.as_str(), asked.path.as_str()) {
                ("POST", path) => self.route(asked, path, &body),
                ("GET" | "HEAD", path) => web::file(&self.web, path),
                _ => Answer::text(405, "not a method this server answers"),
            };
        };
        let caller = match own.gate() {
            Gate::Anyone => None,
            Gate::Signed | Gate::Person => self.store.who(asked.bearer()),
        };
        if let Some(refused) = own.gate().refuses(caller.as_ref()) {
            return refused;
        }
        match (own, caller.as_ref()) {
            (Own::Config, _) => Answer::json(
                200,
                json!({"issuer": self.issuer.url(), "client_id": self.issuer.client()}).to_string(),
            ),
            (Own::SignIn, _) => self.sign_in(&body),
            (Own::OpenApi, _) => Answer::json(200, self.openworks.openapi()),
            (Own::Tokens, Some(caller)) => self.mint(caller),
            (Own::SignOut, Some(caller)) => self.sign_out(caller, asked.bearer()),
            (Own::Inbox, Some(caller)) => self.inbox(caller),
            (Own::Close, Some(caller)) => self.close(asked, caller),
            (Own::Tokens | Own::SignOut | Own::Inbox | Own::Close, None) => Answer::text(401, "sign in"),
        }
    }

    /// Adds the first org and its owner; once there is an org it does nothing and answers that org's id.
    pub fn init(&self, name: &str, owner: &str) -> Result<Id, Fail> {
        let by = Id("init".to_owned());
        let mut org = None;
        self.store.transact(&mut |tx| {
            if let Some(Value::Text(found)) = tx.run(openworks_db::orgs::ALL, &[])?.into_iter().flatten().next() {
                org = Some((Id(found), false));
                return Ok(());
            }
            let id = self.store.id();
            let at = self.store.now();
            tx.run(ORG, &[text(&id.0), text(name.trim()), Value::Int(at)])?;
            self.trail(tx, &id, &by, "server.init", name.trim())?;
            org = Some((id, true));
            Ok(())
        })?;
        let (org, added) = org.ok_or_else(|| Fail::Store("init wrote nothing".to_owned()))?;
        if added {
            self.openworks.set_member(by, org.clone(), owner, Role::Owner)?;
        }
        Ok(org)
    }

    fn reply(&self, stream: &TcpStream) {
        if stream.set_read_timeout(Some(READ_TIMEOUT)).is_err() {
            return;
        }
        let answer = match http::read(stream) {
            Ok(asked) => self.answer(&asked),
            Err(refused) => refused,
        };
        if let Err(error) = http::write(stream, &answer) {
            log(&format!("reply: {error}"));
        }
    }

    fn sign_in(&self, body: &str) -> Answer {
        let Ok(asked) = serde_json::from_str::<Json>(body) else {
            return Answer::text(400, "the body is a JSON object of id_token and org");
        };
        let field = |name: &str| asked.get(name).and_then(Json::as_str).unwrap_or_default();
        let signed = match self.issuer.check(field("id_token"), self.store.now() / 1000) {
            Ok(signed) => signed,
            Err(why) => return Answer::text(401, &why),
        };
        let caller = Caller {
            person: Id(signed.subject),
            email: signed.email.trim().to_lowercase(),
            org: Id(field("org").to_owned()),
            via: Via::Person,
        };
        let mut token = String::new();
        self.write(|tx| {
            let member = tx.run(
                MEMBER,
                &[text(&caller.org.0), text(&caller.person.0), text(&caller.email)],
            )?;
            if member.is_empty() {
                return Err(Fail::NotMember);
            }
            token = tokens::issue(tx, &caller, self.store.now())?;
            self.trail(tx, &caller.org, &caller.person, "server.sign-in", &caller.email)
        })
        .map_or_else(
            |fail| failed(&fail),
            |()| Answer::json(200, json!({"token": token}).to_string()),
        )
    }

    fn mint(&self, caller: &Caller) -> Answer {
        let assistant = Caller {
            via: Via::Assistant,
            ..caller.clone()
        };
        let mut token = String::new();
        self.write(|tx| {
            token = tokens::issue(tx, &assistant, self.store.now())?;
            self.trail(tx, &caller.org, &caller.person, "server.token", "assistant")
        })
        .map_or_else(
            |fail| failed(&fail),
            |()| Answer::json(200, json!({"token": token}).to_string()),
        )
    }

    fn sign_out(&self, caller: &Caller, token: &str) -> Answer {
        self.write(|tx| {
            tokens::revoke(tx, caller, token)?;
            self.trail(tx, &caller.org, &caller.person, "server.sign-out", "")
        })
        .map_or_else(|fail| failed(&fail), |()| Answer::text(200, "signed out"))
    }

    fn route(&self, asked: &Asked, path: &str, body: &str) -> Answer {
        if let Some(caller) = self
            .store
            .who(asked.bearer())
            .filter(|caller| caller.via == Via::Assistant)
        {
            let offer = self
                .openworks
                .tools()
                .into_iter()
                .find(|tool| tool.path == path)
                .map(|tool| tool.offer);
            match offer {
                Some(Offer::Read) => {}
                Some(Offer::Proposal) => return self.propose(&caller, path, body),
                Some(Offer::ByHand) | None => return Answer::text(403, "not offered to an assistant"),
            }
        }
        let response = self.openworks.handle(&Request {
            path: path.to_owned(),
            credential: asked.bearer().to_owned(),
            idempotency_key: asked.header("idempotency-key").map(str::to_owned),
            body: body.to_owned(),
        });
        answered(response.status, response.body)
    }

    fn propose(&self, caller: &Caller, path: &str, body: &str) -> Answer {
        let id = self.store.id();
        self.write(|tx| {
            let at = self.store.now();
            tx.run(
                PROPOSE,
                &[
                    text(&id.0),
                    text(&caller.org.0),
                    text(&caller.person.0),
                    text(path),
                    text(body),
                    Value::Int(at),
                ],
            )?;
            self.trail(tx, &caller.org, &caller.person, "inbox.propose", path)
        })
        .map_or_else(
            |fail| failed(&fail),
            |()| Answer::json(202, json!({"proposal": id.0}).to_string()),
        )
    }

    fn inbox(&self, caller: &Caller) -> Answer {
        let mut rows = Vec::new();
        let read = self.store.transact(&mut |tx| {
            rows = tx.run(OPEN, &[text(&caller.org.0), text(&caller.person.0)])?;
            Ok(())
        });
        if let Err(fail) = read {
            return failed(&fail);
        }
        let proposals: Vec<Json> = rows
            .into_iter()
            .filter_map(|row| match <[Value; 4]>::try_from(row) {
                Ok([Value::Text(id), Value::Text(path), Value::Text(body), Value::Int(at)]) => {
                    Some(json!({"id": id, "path": path, "body": body, "at": at}))
                }
                _ => None,
            })
            .collect();
        Answer::json(200, Json::Array(proposals).to_string())
    }

    fn close(&self, asked: &Asked, caller: &Caller) -> Answer {
        let rest = asked.path.trim_start_matches("/inbox/");
        let (id, state) = match rest.rsplit_once('/') {
            Some((id, "confirm")) => (id, "done"),
            Some((id, "decline")) => (id, "declined"),
            _ => return Answer::text(404, "no such operation"),
        };
        let mut proposal = None;
        let closed = self.write(|tx| {
            let rows = tx.run(
                CLOSE,
                &[text(id), text(&caller.org.0), text(&caller.person.0), text(state)],
            )?;
            proposal = match rows.into_iter().next().as_deref() {
                Some([Value::Text(path), Value::Text(body)]) => Some((path.clone(), body.clone())),
                _ => return Err(Fail::NotFound),
            };
            if state == "declined" {
                self.trail(tx, &caller.org, &caller.person, "inbox.decline", id)?;
            }
            Ok(())
        });
        if let Err(fail) = closed {
            return failed(&fail);
        }
        let Some((path, body)) = proposal else {
            return Answer::text(404, "no such proposal");
        };
        if state == "declined" {
            return Answer::text(200, "declined");
        }
        let response = self.openworks.handle(&Request {
            path,
            credential: asked.bearer().to_owned(),
            idempotency_key: Some(id.to_owned()),
            body,
        });
        if response.status != 200 {
            let reopened = self
                .store
                .transact(&mut |tx| tx.run(REOPEN, &[text(id), text(&caller.org.0)]).map(drop));
            if let Err(fail) = reopened {
                log(&format!("reopen {id}: {fail}"));
            }
        }
        answered(response.status, response.body)
    }

    fn write(&self, mut work: impl FnMut(&mut dyn Tx) -> Result<(), Fail>) -> Result<(), Fail> {
        self.store.transact(&mut work)
    }

    fn trail(&self, tx: &mut dyn Tx, org: &Id, person: &Id, operation: &str, body: &str) -> Result<(), Fail> {
        let at = Value::Int(self.store.now());
        tx.run(
            openworks_db::AUDIT,
            &[
                text(&self.store.id().0),
                text(&org.0),
                text(&person.0),
                text(operation),
                at.clone(),
            ],
        )?;
        tx.run(
            openworks_db::EVENT,
            &[text(&self.store.id().0), text(&org.0), text(operation), text(body), at],
        )?;
        Ok(())
    }
}

fn answered(status: u16, body: String) -> Answer {
    if status == 200 && (body.starts_with('{') || body.starts_with('[')) {
        Answer::json(status, body)
    } else {
        Answer::text(status, &body)
    }
}

fn failed(fail: &Fail) -> Answer {
    let status = match fail {
        Fail::NotFound => 404,
        Fail::NotMember | Fail::Needs(_) => 403,
        Fail::Invalid(_) => 400,
        Fail::Undeclared(_) | Fail::Store(_) => 500,
    };
    Answer::text(status, &fail.to_string())
}

fn text(value: &str) -> Value {
    Value::Text(value.to_owned())
}

pub(crate) fn log(line: &str) {
    writeln!(std::io::stderr(), "{line}").ok();
}
