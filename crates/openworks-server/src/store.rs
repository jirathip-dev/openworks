use std::cell::{Cell, RefCell};
use std::fmt::Write as _;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use openworks::{Caller, Files, Host, Via};
use openworks_core::{Fail, Id, Row, Tx, Value};
use postgres::types::{ToSql, Type};
use postgres::{Client, NoTls, Transaction};

use crate::tokens;

const SCHEMA: &str = include_str!("server.sql");

/// A host over one Postgres database and a folder of files, one transaction at a time.
pub struct Store {
    client: RefCell<Client>,
    files: Folder,
    prefix: String,
    issued: Cell<u64>,
}

struct Folder(PathBuf);

struct Pg<'a, 'b>(&'a mut Transaction<'b>);

impl Store {
    /// Connects to the database at `url`, keeping files under `files`.
    pub fn connect(url: &str, files: PathBuf) -> Result<Self, Fail> {
        let client = Client::connect(url, NoTls).map_err(stored)?;
        Ok(Self {
            client: RefCell::new(client),
            files: Folder(files),
            prefix: hex(&random::<8>()?),
            issued: Cell::new(0),
        })
    }

    /// Runs the server's own migrations, after the core's.
    pub fn migrate(&self) -> Result<(), Fail> {
        self.transact(&mut |tx| {
            openworks_db::migrate(tx)?;
            for statement in SCHEMA.split(';').map(str::trim) {
                if !statement.is_empty() {
                    tx.run(statement, &[])?;
                }
            }
            Ok(())
        })
    }
}

impl Host for &Store {
    fn who(&self, credential: &str) -> Option<Caller> {
        let mut found = None;
        self.transact(&mut |tx| {
            found = tokens::find(tx, credential)?;
            Ok(())
        })
        .ok()?;
        found
    }

    fn transact(&self, work: &mut dyn FnMut(&mut dyn Tx) -> Result<(), Fail>) -> Result<(), Fail> {
        let mut client = self
            .client
            .try_borrow_mut()
            .map_err(|_| Fail::Store("a transaction is open already".to_owned()))?;
        let mut transaction = client.transaction().map_err(stored)?;
        work(&mut Pg(&mut transaction))?;
        transaction.commit().map_err(stored)
    }

    fn id(&self) -> Id {
        let issued = self.issued.get() + 1;
        self.issued.set(issued);
        Id(format!("{}{issued:x}", self.prefix))
    }

    fn now(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| i64::try_from(since.as_millis()).unwrap_or(i64::MAX))
    }

    fn files(&self) -> &dyn Files {
        &self.files
    }

    fn name(&self, _: &str) -> Option<String> {
        None
    }
}

impl Tx for Pg<'_, '_> {
    fn run(&mut self, sql: &'static str, params: &[Value]) -> Result<Vec<Row>, Fail> {
        let statement = self.0.prepare(sql).map_err(stored)?;
        if statement.params().len() != params.len() {
            return Err(Fail::Store(format!("{sql} takes {} values", statement.params().len())));
        }
        let bound: Vec<Box<dyn ToSql + Sync>> = params
            .iter()
            .zip(statement.params())
            .map(|(value, kind)| bound(value, kind))
            .collect();
        let refs: Vec<&(dyn ToSql + Sync)> = bound.iter().map(AsRef::as_ref).collect();
        let rows = self.0.query(&statement, &refs).map_err(stored)?;
        rows.iter().map(read).collect()
    }
}

impl Files for Folder {
    fn put(&self, key: &str, bytes: &[u8]) -> Result<(), Fail> {
        std::fs::create_dir_all(&self.0).map_err(stored)?;
        std::fs::write(self.0.join(hex(key.as_bytes())), bytes).map_err(stored)
    }

    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, Fail> {
        match std::fs::read(self.0.join(hex(key.as_bytes()))) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(stored(error)),
        }
    }
}

fn bound(value: &Value, kind: &Type) -> Box<dyn ToSql + Sync> {
    match value {
        Value::Int(number) if *kind == Type::INT4 => match i32::try_from(*number) {
            Ok(small) => Box::new(small),
            Err(_) => Box::new(*number),
        },
        Value::Int(number) => Box::new(*number),
        Value::Text(text) => Box::new(text.clone()),
        Value::Null if *kind == Type::INT8 => Box::new(None::<i64>),
        Value::Null if *kind == Type::INT4 => Box::new(None::<i32>),
        Value::Null => Box::new(None::<String>),
    }
}

fn read(row: &postgres::Row) -> Result<Row, Fail> {
    row.columns()
        .iter()
        .enumerate()
        .map(|(at, column)| {
            let kind = column.type_();
            let value = if *kind == Type::INT8 {
                row.try_get::<_, Option<i64>>(at).map(|number| number.map(Value::Int))
            } else if *kind == Type::INT4 {
                row.try_get::<_, Option<i32>>(at)
                    .map(|number| number.map(|number| Value::Int(i64::from(number))))
            } else if *kind == Type::BOOL {
                row.try_get::<_, Option<bool>>(at)
                    .map(|yes| yes.map(|yes| Value::Int(i64::from(yes))))
            } else {
                row.try_get::<_, Option<String>>(at).map(|text| text.map(Value::Text))
            };
            value.map(|value| value.unwrap_or(Value::Null)).map_err(stored)
        })
        .collect()
}

pub(crate) fn random<const N: usize>() -> Result<[u8; N], Fail> {
    let mut bytes = [0; N];
    getrandom::fill(&mut bytes).map_err(|error| Fail::Store(error.to_string()))?;
    Ok(bytes)
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut text, byte| {
        if write!(text, "{byte:02x}").is_err() {
            text.clear();
        }
        text
    })
}

fn stored(error: impl std::fmt::Display) -> Fail {
    Fail::Store(error.to_string())
}

pub(crate) fn via(caller: &Caller) -> &'static str {
    match caller.via {
        Via::Person => "person",
        Via::Assistant => "assistant",
    }
}
