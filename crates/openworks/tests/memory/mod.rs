use std::cell::{Cell, RefCell};

use openworks::{Caller, Files, Host, Openworks, Request, Response, Via};
use openworks_core::{Fail, Id, Tx};

use crate::common::NoFiles;

/// The time a test host's clock starts at.
pub const T0: i64 = 1_700_000_000_000;

/// A host over an in-memory store; `other` signs in to org `o2`, anyone else to `o1`.
#[derive(Default)]
pub struct TestHost<S> {
    pub committed: RefCell<S>,
    pub next_id: Cell<u32>,
    pub clock: Cell<i64>,
}

impl<S: Tx + Clone> Host for &TestHost<S> {
    fn who(&self, credential: &str) -> Option<Caller> {
        let org = if credential == "other" { "o2" } else { "o1" };
        Some(Caller {
            person: Id(credential.to_owned()),
            email: format!("{credential}@o.test"),
            org: Id(org.to_owned()),
            via: Via::Person,
        })
    }

    fn transact(&self, work: &mut dyn FnMut(&mut dyn Tx) -> Result<(), Fail>) -> Result<(), Fail> {
        let mut pending = self.committed.borrow().clone();
        work(&mut pending)?;
        *self.committed.borrow_mut() = pending;
        Ok(())
    }

    fn id(&self) -> Id {
        self.next_id.set(self.next_id.get() + 1);
        Id(format!("id{}", self.next_id.get()))
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

/// Sends a body to a path as a credential, keyed by both.
pub fn send<S: Tx + Clone>(openworks: &Openworks<&TestHost<S>>, path: &str, credential: &str, body: &str) -> Response {
    openworks.handle(&Request {
        path: path.to_owned(),
        credential: credential.to_owned(),
        idempotency_key: Some(format!("{path} {body}")),
        body: body.to_owned(),
    })
}

/// The body of an answer that must be a 200.
pub fn ok(response: Response) -> String {
    assert_eq!(response.status, 200, "{}", response.body);
    response.body
}
