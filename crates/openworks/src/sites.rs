use openworks_core::{Call, Declaration, Fail, Id, Record, Row, Tx, Value, Who};
use openworks_db::clients::GET as CLIENT;
use openworks_db::sites::{ADD, DEACTIVATE as DEACTIVATE_SQL, GET as GET_SQL, LIST as LIST_SQL, SAVE as SAVE_SQL};

use crate::clients::{changed, found, lines, named, org};

/// Lists the sites of the client with the body's id, as records of kind `sites`.
pub const LIST: Declaration = openworks_core::list!("list", needs: Read, record: "sites", queries: [LIST_SQL], list);

/// Reads the site with the body's id, as a page of one record of kind `sites`.
pub const GET: Declaration = openworks_core::list!("get", needs: Read, record: "sites", queries: [GET_SQL], get);

/// Saves a site from a body of `id`, `client_id`, `name`, `address` and `map_url` lines; an empty id adds one; answers its id.
pub const SAVE: Declaration = openworks_core::create!("save", needs: Write, queries: [CLIENT, ADD, SAVE_SQL], save);

/// Deactivates the site with the body's id.
pub const DEACTIVATE: Declaration =
    openworks_core::write!("deactivate", needs: Write, queries: [DEACTIVATE_SQL], deactivate);

fn list(who: &Who, tx: &mut dyn Tx, body: &str) -> Result<Vec<Record>, Fail> {
    let client = Value::Text(body.trim().to_owned());
    tx.run(LIST_SQL, &[org(who), client])?.into_iter().map(record).collect()
}

fn get(who: &Who, tx: &mut dyn Tx, body: &str) -> Result<Vec<Record>, Fail> {
    let rows = tx.run(GET_SQL, &[org(who), Value::Text(body.trim().to_owned())])?;
    found(rows.into_iter().map(record).collect::<Result<_, _>>()?)
}

fn save(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let [id, client, name, address, map_url] = lines(call.body)?;
    if !(map_url.is_empty() || map_url.starts_with("https://") || map_url.starts_with("http://")) {
        return Err(Fail::Invalid(format!("not a map link: {map_url}")));
    }
    let client = Value::Text(client.to_owned());
    let (sql, site) = if id.is_empty() {
        if tx.run(CLIENT, &[org(call.who), client.clone()])?.is_empty() {
            return Err(Fail::NotFound);
        }
        (ADD, call.id.0.as_str())
    } else {
        (SAVE_SQL, id)
    };
    let params = [
        org(call.who),
        Value::Text(site.to_owned()),
        client,
        named(name)?,
        Value::Text(address.to_owned()),
        Value::Text(map_url.to_owned()),
    ];
    let rows = tx.run(sql, &params)?;
    if sql == ADD { Ok(site.to_owned()) } else { changed(rows) }
}

fn deactivate(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let id = Value::Text(call.body.trim().to_owned());
    changed(tx.run(DEACTIVATE_SQL, &[org(call.who), id, Value::Int(call.at)])?)
}

fn record(row: Row) -> Result<Record, Fail> {
    match <[Value; 6]>::try_from(row) {
        Ok([Value::Text(id), client, name, address, map_url, deactivated_at]) => Ok(Record {
            id: Id(id),
            fields: vec![
                ("client_id", client),
                ("name", name),
                ("address", address),
                ("map_url", map_url),
                ("deactivated_at", deactivated_at),
            ],
        }),
        _ => Err(Fail::Store(
            "a site row is its id, client_id, name, address, map_url and deactivated_at".to_owned(),
        )),
    }
}
