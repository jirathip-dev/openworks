use std::fmt;

/// An opaque identifier the host hands out.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(pub String);

/// A permission word a person holds and an operation needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Word(pub &'static str);

/// The signed-in person, the org they act for and the words they hold.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Who {
    pub person: Id,
    pub org: Id,
    pub words: Vec<String>,
}

impl Who {
    /// Whether this person holds the word.
    pub fn holds(&self, word: Word) -> bool {
        self.words.iter().any(|held| held == word.0)
    }
}

/// A value bound to a query parameter or read from a row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Null,
    Int(i64),
    Text(String),
}

/// One row a query returns.
pub type Row = Vec<Value>;

/// Why an operation did not complete.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fail {
    NotFound,
    Invalid(String),
    Undeclared(&'static str),
    Store(String),
}

impl fmt::Display for Fail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("not found"),
            Self::Invalid(why) => std::write!(f, "invalid: {why}"),
            Self::Undeclared(sql) => std::write!(f, "undeclared query: {sql}"),
            Self::Store(why) => std::write!(f, "store: {why}"),
        }
    }
}

/// A transaction the host opens: every statement in it commits or none does.
pub trait Tx {
    fn run(&mut self, sql: &'static str, params: &[Value]) -> Result<Vec<Row>, Fail>;
}

/// Files the host stores by key.
pub trait Files {
    fn put(&self, key: &str, bytes: &[u8]) -> Result<(), Fail>;
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, Fail>;
}

/// What a store handler is called with: the caller, the body, the change's time, a fresh id and the host's files.
pub struct Call<'a> {
    pub who: &'a Who,
    pub body: &'a str,
    pub at: i64,
    pub id: Id,
    pub files: &'a dyn Files,
}

/// What an operation does to the store.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Read,
    Write,
    Create,
    Closed,
}

/// The body of an operation, by what it may touch.
#[derive(Clone, Copy)]
pub enum Handler {
    Store(fn(&Call, &mut dyn Tx) -> Result<String, Fail>),
    Closed(fn(&Who, &str) -> Result<String, Fail>),
}

/// An operation's declaration: its name, kind, the word it needs, its queries and its body.
#[derive(Clone, Copy)]
pub struct Declaration {
    name: &'static str,
    kind: Kind,
    needs: Word,
    queries: &'static [&'static str],
    handler: Handler,
}

impl Declaration {
    /// A read: runs its queries in a transaction and leaves nothing behind.
    pub const fn read(
        name: &'static str,
        needs: Word,
        queries: &'static [&'static str],
        handler: fn(&Call, &mut dyn Tx) -> Result<String, Fail>,
    ) -> Self {
        Self {
            name,
            kind: Kind::Read,
            needs,
            queries,
            handler: Handler::Store(handler),
        }
    }

    /// A write: runs its queries in a transaction that also leaves one audit row and one event.
    pub const fn write(
        name: &'static str,
        needs: Word,
        queries: &'static [&'static str],
        handler: fn(&Call, &mut dyn Tx) -> Result<String, Fail>,
    ) -> Self {
        Self {
            name,
            kind: Kind::Write,
            needs,
            queries,
            handler: Handler::Store(handler),
        }
    }

    /// A create: a write that runs once per `Idempotency-Key` and answers a repeat with its first answer.
    pub const fn create(
        name: &'static str,
        needs: Word,
        queries: &'static [&'static str],
        handler: fn(&Call, &mut dyn Tx) -> Result<String, Fail>,
    ) -> Self {
        Self {
            name,
            kind: Kind::Create,
            needs,
            queries,
            handler: Handler::Store(handler),
        }
    }

    /// A closed operation: answers from who and the input alone, with no store.
    pub const fn closed(name: &'static str, needs: Word, handler: fn(&Who, &str) -> Result<String, Fail>) -> Self {
        Self {
            name,
            kind: Kind::Closed,
            needs,
            queries: &[],
            handler: Handler::Closed(handler),
        }
    }

    /// The operation's name, its path under a mount's prefix.
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// What the operation does to the store.
    pub const fn kind(&self) -> Kind {
        self.kind
    }

    /// The word a caller must hold.
    pub const fn needs(&self) -> Word {
        self.needs
    }

    /// The only queries the operation may run.
    pub const fn queries(&self) -> &'static [&'static str] {
        self.queries
    }

    /// The operation's body.
    pub const fn handler(&self) -> Handler {
        self.handler
    }
}

#[macro_export]
macro_rules! read {
    ($name:literal, needs: $word:literal, queries: [$($query:expr),* $(,)?], $handler:expr $(,)?) => {
        $crate::Declaration::read($name, $crate::Word($word), &[$($query),*], $handler)
    };
}

#[macro_export]
macro_rules! write {
    ($name:literal, needs: $word:literal, queries: [$($query:expr),* $(,)?], $handler:expr $(,)?) => {
        $crate::Declaration::write($name, $crate::Word($word), &[$($query),*], $handler)
    };
}

#[macro_export]
macro_rules! create {
    ($name:literal, needs: $word:literal, queries: [$($query:expr),* $(,)?], $handler:expr $(,)?) => {
        $crate::Declaration::create($name, $crate::Word($word), &[$($query),*], $handler)
    };
}

#[macro_export]
macro_rules! closed {
    ($name:literal, needs: $word:literal, $handler:expr $(,)?) => {
        $crate::Declaration::closed($name, $crate::Word($word), $handler)
    };
}

#[macro_export]
macro_rules! operations {
    ($($variant:ident => $declaration:path),+ $(,)?) => {
        /// Every operation this crate declares.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum Operation {
            $($variant),+
        }

        impl Operation {
            /// Every operation, in declaration order.
            pub const ALL: &'static [Operation] = &[$(Operation::$variant),+];

            /// The operation's declaration.
            pub const fn declaration(self) -> &'static $crate::Declaration {
                match self {
                    $(Operation::$variant => &$declaration),+
                }
            }
        }
    };
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

#[cfg(doctest)]
#[doc = include_str!("../tests/undeclared.md")]
struct Undeclared;
