use std::fmt;

/// An opaque identifier the host hands out.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(pub String);

/// One of the five words an operation needs and a role grants.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Word {
    Read,
    Write,
    Approve,
    Members,
    Settings,
}

impl Word {
    /// Every word.
    pub const ALL: [Self; 5] = [Self::Read, Self::Write, Self::Approve, Self::Members, Self::Settings];

    /// The word as a caller sees it.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Approve => "approve",
            Self::Members => "members",
            Self::Settings => "settings",
        }
    }
}

/// How far a granted word reaches: every record in the org, or only the person's own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Scope {
    All,
    Own,
}

/// A member's role in an org.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    Owner,
    Manager,
    Member,
    Viewer,
}

impl Role {
    /// Every role.
    pub const ALL: [Self; 4] = [Self::Owner, Self::Manager, Self::Member, Self::Viewer];

    /// The role as it is stored and read.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Manager => "manager",
            Self::Member => "member",
            Self::Viewer => "viewer",
        }
    }

    /// The role with this name.
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|role| role.name() == name)
    }

    /// The role table: how far this role holds the word, if at all.
    pub const fn grants(self, word: Word) -> Option<Scope> {
        match (self, word) {
            (Self::Owner, _)
            | (Self::Manager, Word::Read | Word::Write | Word::Approve)
            | (Self::Member | Self::Viewer, Word::Read) => Some(Scope::All),
            (Self::Member, Word::Write) => Some(Scope::Own),
            _ => None,
        }
    }
}

/// The signed-in member, the org they act for and their role in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Who {
    pub person: Id,
    pub org: Id,
    pub role: Role,
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
    NotMember,
    Needs(Word),
    Invalid(String),
    Undeclared(&'static str),
    Store(String),
}

impl fmt::Display for Fail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("not found"),
            Self::NotMember => f.write_str("not a member"),
            Self::Needs(word) => std::write!(f, "needs {}", word.name()),
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

/// What an operation does to the store.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Read,
    Write,
    Closed,
}

/// The body of an operation, by what it may touch.
#[derive(Clone, Copy)]
pub enum Handler {
    Store(fn(&Who, &mut dyn Tx, &str) -> Result<String, Fail>),
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
        handler: fn(&Who, &mut dyn Tx, &str) -> Result<String, Fail>,
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
        handler: fn(&Who, &mut dyn Tx, &str) -> Result<String, Fail>,
    ) -> Self {
        Self {
            name,
            kind: Kind::Write,
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
    ($name:literal, needs: $word:ident, queries: [$($query:expr),* $(,)?], $handler:expr $(,)?) => {
        $crate::Declaration::read($name, $crate::Word::$word, &[$($query),*], $handler)
    };
}

#[macro_export]
macro_rules! write {
    ($name:literal, needs: $word:ident, queries: [$($query:expr),* $(,)?], $handler:expr $(,)?) => {
        $crate::Declaration::write($name, $crate::Word::$word, &[$($query),*], $handler)
    };
}

#[macro_export]
macro_rules! closed {
    ($name:literal, needs: $word:ident, $handler:expr $(,)?) => {
        $crate::Declaration::closed($name, $crate::Word::$word, $handler)
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
