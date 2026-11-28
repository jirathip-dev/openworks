use crate::{Approval, Fail, Files, Id, Record, Tx, Who, Word};

/// The body of a list read: the records of one page.
pub type Lister = fn(&Who, &mut dyn Tx, &str) -> Result<Vec<Record>, Fail>;

/// What a store handler is called with: the caller, the body, the change's time, a fresh id each time it asks, the host's files and the packs' approval kinds.
pub struct Call<'a> {
    pub who: &'a Who,
    pub body: &'a str,
    pub at: i64,
    pub fresh: &'a dyn Fn() -> Id,
    pub files: &'a dyn Files,
    pub approvals: &'a [Approval],
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
    List(&'static str, Lister),
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
    by_hand: bool,
}

/// What an assistant is offered of an operation: a read tool, a proposal a person confirms, or nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Offer {
    Read,
    Proposal,
    ByHand,
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
            by_hand: false,
        }
    }

    /// A list read: returns records of one kind, each with every extension on that kind under `ext`.
    pub const fn list(
        name: &'static str,
        needs: Word,
        record: &'static str,
        queries: &'static [&'static str],
        handler: Lister,
    ) -> Self {
        Self {
            name,
            kind: Kind::Read,
            needs,
            queries,
            handler: Handler::List(record, handler),
            by_hand: false,
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
            by_hand: false,
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
            by_hand: false,
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
            by_hand: false,
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

    /// The same operation, never offered to an assistant: a delete, a file or a contact.
    #[must_use]
    pub const fn by_hand(self) -> Self {
        Self { by_hand: true, ..self }
    }

    /// What an assistant is offered: members and settings are only ever by hand, a write only as a proposal.
    pub const fn offer(&self) -> Offer {
        match (self.by_hand, self.kind, self.needs) {
            (true, _, _) | (_, _, Word::Members | Word::Settings) => Offer::ByHand,
            (false, Kind::Read | Kind::Closed, _) => Offer::Read,
            (false, Kind::Write | Kind::Create, _) => Offer::Proposal,
        }
    }
}

#[macro_export]
macro_rules! read {
    ($name:literal, needs: $word:ident, queries: [$($query:expr),* $(,)?], $handler:expr $(,)?) => {
        $crate::Declaration::read($name, $crate::Word::$word, &[$($query),*], $handler)
    };
}

#[macro_export]
macro_rules! list {
    ($name:literal, needs: $word:ident, record: $record:literal, queries: [$($query:expr),* $(,)?], $handler:expr $(,)?) => {
        $crate::Declaration::list($name, $crate::Word::$word, $record, &[$($query),*], $handler)
    };
}

#[macro_export]
macro_rules! write {
    ($name:literal, needs: $word:ident, queries: [$($query:expr),* $(,)?], $handler:expr $(,)?) => {
        $crate::Declaration::write($name, $crate::Word::$word, &[$($query),*], $handler)
    };
}

#[macro_export]
macro_rules! create {
    ($name:literal, needs: $word:ident, queries: [$($query:expr),* $(,)?], $handler:expr $(,)?) => {
        $crate::Declaration::create($name, $crate::Word::$word, &[$($query),*], $handler)
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
