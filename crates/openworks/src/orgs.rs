use openworks_core::{Call, Declaration, Fail, Id, Record, Tx, Value, Who};
use openworks_db::orgs::{LIST as LIST_SQL, RENAME as RENAME_SQL};

/// Lists the caller's orgs as records of kind `orgs`.
pub const LIST: Declaration = openworks_core::list!("list", needs: Read, record: "orgs", queries: [LIST_SQL], list);

/// Renames the caller's org to the body.
pub const RENAME: Declaration = openworks_core::write!("rename", needs: Settings, queries: [RENAME_SQL], rename);

fn list(who: &Who, tx: &mut dyn Tx, _: &str) -> Result<Vec<Record>, Fail> {
    let rows = tx.run(LIST_SQL, &[Value::Text(who.org.0.clone())])?;
    rows.into_iter()
        .map(|row| match <[Value; 2]>::try_from(row) {
            Ok([Value::Text(id), name]) => Ok(Record {
                id: Id(id),
                fields: vec![("name", name)],
            }),
            _ => Err(Fail::Store("an org row is its id and name".to_owned())),
        })
        .collect()
}

fn rename(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let name = call.body.trim();
    if name.is_empty() {
        return Err(Fail::Invalid("an org needs a name".to_owned()));
    }
    tx.run(
        RENAME_SQL,
        &[Value::Text(call.who.org.0.clone()), Value::Text(name.to_owned())],
    )?;
    Ok(name.to_owned())
}
