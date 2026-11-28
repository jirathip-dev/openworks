use openworks_core::{Call, Declaration, Fail, Id, Mount, Reach, Record, Row, Tx, Value, Who};
use openworks_db::clients::{
    ADD, DEACTIVATE as DEACTIVATE_SQL, GET as GET_SQL, LIST as LIST_SQL, SAVE as SAVE_SQL, SYNC as SYNC_SQL,
};

use crate::{Host, Openworks};

/// Lists the org's clients as records of kind `clients`.
pub const LIST: Declaration = openworks_core::list!("list", needs: Read, record: "clients", queries: [LIST_SQL], list);

/// Reads the client with the body's id, as a page of one record of kind `clients`.
pub const GET: Declaration = openworks_core::list!("get", needs: Read, record: "clients", queries: [GET_SQL], get);

/// Saves a client from a body of `id`, `name` and `host_ref` lines; an empty id adds one; answers its id.
pub const SAVE: Declaration = openworks_core::create!("save", needs: Write, queries: [ADD, SAVE_SQL], save);

/// Deactivates the client with the body's id.
pub const DEACTIVATE: Declaration =
    openworks_core::write!("deactivate", needs: Write, queries: [DEACTIVATE_SQL], deactivate);

/// Adds or renames the client a host knows by reference, from a body of `host_ref` and `name` lines; answers its id.
pub const SYNC: Declaration = openworks_core::write!("sync", needs: Write, queries: [SYNC_SQL], sync);

fn list(who: &Who, tx: &mut dyn Tx, _: &str) -> Result<Vec<Record>, Fail> {
    tx.run(LIST_SQL, &[org(who)])?.into_iter().map(record).collect()
}

fn get(who: &Who, tx: &mut dyn Tx, body: &str) -> Result<Vec<Record>, Fail> {
    let rows = tx.run(GET_SQL, &[org(who), Value::Text(body.trim().to_owned())])?;
    found(rows.into_iter().map(record).collect::<Result<_, _>>()?)
}

fn save(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let [id, name, host_ref] = lines(call.body)?;
    let (name, host_ref) = (named(name)?, optional(host_ref));
    if id.is_empty() {
        let id = (call.fresh)().0;
        tx.run(ADD, &[org(call.who), Value::Text(id.clone()), name, host_ref])?;
        return Ok(id);
    }
    changed(tx.run(SAVE_SQL, &[org(call.who), Value::Text(id.to_owned()), name, host_ref])?)
}

fn deactivate(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let id = Value::Text(call.body.trim().to_owned());
    changed(tx.run(DEACTIVATE_SQL, &[org(call.who), id, Value::Int(call.at)])?)
}

fn sync(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let [host_ref, name] = lines(call.body)?;
    if host_ref.is_empty() {
        return Err(Fail::Invalid("a synced client needs a host_ref".to_owned()));
    }
    let params = [
        org(call.who),
        Value::Text((call.fresh)().0),
        named(name)?,
        Value::Text(host_ref.to_owned()),
    ];
    changed(tx.run(SYNC_SQL, &params)?)
}

fn record(row: Row) -> Result<Record, Fail> {
    match <[Value; 4]>::try_from(row) {
        Ok([Value::Text(id), name, host_ref, deactivated_at]) => Ok(Record {
            id: Id(id),
            fields: vec![
                ("name", name),
                ("host_ref", host_ref),
                ("deactivated_at", deactivated_at),
            ],
        }),
        _ => Err(Fail::Store(
            "a client row is its id, name, host_ref and deactivated_at".to_owned(),
        )),
    }
}

pub(crate) fn lines<const N: usize>(body: &str) -> Result<[&str; N], Fail> {
    let lines: Vec<&str> = body.split('\n').map(str::trim).collect();
    if lines.len() > N {
        return Err(Fail::Invalid(format!("expected at most {N} lines")));
    }
    Ok(std::array::from_fn(|at| lines.get(at).copied().unwrap_or("")))
}

pub(crate) fn named(name: &str) -> Result<Value, Fail> {
    if name.is_empty() {
        Err(Fail::Invalid("a name is needed".to_owned()))
    } else {
        Ok(Value::Text(name.to_owned()))
    }
}

pub(crate) fn optional(text: &str) -> Value {
    if text.is_empty() {
        Value::Null
    } else {
        Value::Text(text.to_owned())
    }
}

pub(crate) fn found(records: Vec<Record>) -> Result<Vec<Record>, Fail> {
    if records.is_empty() {
        Err(Fail::NotFound)
    } else {
        Ok(records)
    }
}

pub(crate) fn changed(rows: Vec<Row>) -> Result<String, Fail> {
    match rows.into_iter().next().and_then(|row| row.into_iter().next()) {
        Some(Value::Text(id)) => Ok(id),
        _ => Err(Fail::NotFound),
    }
}

pub(crate) fn org(who: &Who) -> Value {
    Value::Text(who.org.0.clone())
}

pub(crate) fn mount() -> Mount {
    Mount::new("/clients", "clients")
        .operation(&LIST, Reach::Org)
        .operation(&GET, Reach::Org)
        .operation(&SAVE, Reach::Org)
        .operation(&DEACTIVATE, Reach::Org)
        .operation(&SYNC, Reach::Org)
}

impl<H: Host> Openworks<H> {
    /// Adds the client the host knows by `host_ref`, or renames it, on the host's word; answers the client's id.
    pub fn sync_client(&self, by: Id, org: Id, host_ref: &str, name: &str) -> Result<String, Fail> {
        self.as_host(by, org, &SYNC, "clients", &format!("{host_ref}\n{name}"))
    }
}
