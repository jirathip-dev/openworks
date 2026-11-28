use crate::{Declaration, Fail, Id, Tx, Type, Value, Who, Word};

/// The body of an extension: its values for each record id it knows, in its field order.
pub type Extender = fn(&Who, &mut dyn Tx, &[Id]) -> Result<Vec<(Id, Vec<Value>)>, Fail>;

/// Fields a pack adds to every record of one kind, read as `ext.<key>` in the read's transaction.
#[derive(Clone, Copy)]
pub struct Extension {
    pub record: &'static str,
    pub key: &'static str,
    pub fields: &'static [(&'static str, Type)],
    pub queries: &'static [&'static str],
    pub extend: Extender,
}

/// A kind of approval a pack asks for, the word an approver needs and whether a yes needs a signature file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Approval {
    pub key: &'static str,
    pub needs: Word,
    pub signature: bool,
}

/// A print template a pack brings for one record kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Print {
    pub key: &'static str,
    pub record: &'static str,
    pub template: &'static str,
}

/// What a pack bundles: extensions, hooks, approval kinds, routes and prints.
#[derive(Default)]
pub struct Pack {
    pub extensions: Vec<&'static Extension>,
    pub hooks: Vec<Box<dyn Hook>>,
    pub approvals: Vec<Approval>,
    pub mounts: Vec<Mount>,
    pub prints: Vec<Print>,
}

/// One change a write made, as its event and its hooks see it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub org: Id,
    pub person: Id,
    pub operation: String,
    pub body: String,
    pub at: i64,
}

/// Code run around every change in the order added: an error before commit rolls it back, one after is retried.
pub trait Hook {
    fn before_commit(&self, _change: &Change) -> Result<(), Fail> {
        Ok(())
    }

    fn after_commit(&self, _change: &Change) -> Result<(), Fail> {
        Ok(())
    }
}

/// Who may reach an operation through its mount.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reach {
    Org,
    Off,
}

/// A set of declared operations under one path prefix and one extension key.
#[derive(Clone)]
pub struct Mount {
    prefix: &'static str,
    key: &'static str,
    operations: Vec<(&'static Declaration, Reach)>,
}

impl Mount {
    /// An empty mount at a path prefix, its events and audit rows tagged with the extension key.
    pub fn new(prefix: &'static str, key: &'static str) -> Self {
        Self {
            prefix,
            key,
            operations: Vec::new(),
        }
    }

    /// Adds a declared operation with its reach.
    #[must_use]
    pub fn operation(mut self, declaration: &'static Declaration, reach: Reach) -> Self {
        self.operations.push((declaration, reach));
        self
    }

    /// The path prefix.
    pub fn prefix(&self) -> &'static str {
        self.prefix
    }

    /// The extension key.
    pub fn key(&self) -> &'static str {
        self.key
    }

    /// The declared operations with their reach.
    pub fn operations(&self) -> &[(&'static Declaration, Reach)] {
        &self.operations
    }
}
