use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicI64, AtomicU32, Ordering};

use openworks::{Caller, Files, Host, Openworks, Request, Response, Via};
use openworks_core::{Fail, Id, Tx};

/// A file store that keeps every file in memory by key.
#[derive(Default)]
pub struct Kept(pub Mutex<HashMap<String, Vec<u8>>>);

impl Files for Kept {
    fn put(&self, key: &str, bytes: &[u8]) -> Result<(), Fail> {
        self.0.lock().unwrap().insert(key.to_owned(), bytes.to_vec());
        Ok(())
    }
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, Fail> {
        Ok(self.0.lock().unwrap().get(key).cloned())
    }
}

/// The time a test host's clock starts at.
pub const T0: i64 = 1_700_000_000_000;

/// A host over an in-memory store, one transaction at a time; `other` signs in to org `o2`, anyone else to `o1`.
#[derive(Default)]
pub struct TestHost<S> {
    pub committed: Mutex<S>,
    pub next_id: AtomicU32,
    pub clock: AtomicI64,
    pub files: Kept,
}

impl<S> TestHost<S> {
    /// The committed store.
    pub fn store(&self) -> std::sync::MutexGuard<'_, S> {
        self.committed.lock().unwrap()
    }

    /// Moves the clock to `T0` plus `after`.
    pub fn set_clock(&self, after: i64) {
        self.clock.store(after, Ordering::SeqCst);
    }
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
        let mut committed = self.store();
        let mut pending = committed.clone();
        work(&mut pending)?;
        *committed = pending;
        Ok(())
    }

    fn id(&self) -> Id {
        Id(format!("id{}", self.next_id.fetch_add(1, Ordering::SeqCst) + 1))
    }

    fn now(&self) -> i64 {
        T0 + self.clock.load(Ordering::SeqCst)
    }

    fn files(&self) -> &dyn Files {
        &self.files
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
