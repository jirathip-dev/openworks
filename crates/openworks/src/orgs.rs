use openworks_core::{Call, Declaration, Fail, Tx, Value};
use openworks_db::orgs::{RENAME as RENAME_SQL, SHOW as SHOW_SQL};

/// Reads the caller's org name.
pub const SHOW: Declaration = openworks_core::read!("show", needs: "orgs.read", queries: [SHOW_SQL], show);

/// Renames the caller's org to the body.
pub const RENAME: Declaration = openworks_core::write!("rename", needs: "orgs.write", queries: [RENAME_SQL], rename);

fn show(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let rows = tx.run(SHOW_SQL, &[Value::Text(call.who.org.0.clone())])?;
    match rows.first().and_then(|row| row.first()) {
        Some(Value::Text(name)) => Ok(name.clone()),
        _ => Err(Fail::NotFound),
    }
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
