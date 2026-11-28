use std::fmt;

use crate::{Id, Word};

/// A value bound to a query parameter or read from a row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Null,
    Int(i64),
    Text(String),
}

/// One row a query returns.
pub type Row = Vec<Value>;

/// The type of a field a record or an extension carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type {
    Int,
    Text,
}

/// One record a list read returns: its id and its named fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub id: Id,
    pub fields: Vec<(&'static str, Value)>,
}

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

/// Files the host stores by key.
pub trait Files {
    fn put(&self, key: &str, bytes: &[u8]) -> Result<(), Fail>;
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, Fail>;
}
