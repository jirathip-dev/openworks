use openworks_core::{Declaration, Fail, Tx, Value, Who};
use openworks_db::orgs::{RENAME as RENAME_SQL, SHOW as SHOW_SQL};

/// Reads the caller's org name.
pub const SHOW: Declaration = openworks_core::read!("show", needs: "orgs.read", queries: [SHOW_SQL], show);

/// Renames the caller's org to the body.
pub const RENAME: Declaration = openworks_core::write!("rename", needs: "orgs.write", queries: [RENAME_SQL], rename);

fn show(who: &Who, tx: &mut dyn Tx, _: &str) -> Result<String, Fail> {
    let rows = tx.run(SHOW_SQL, &[Value::Text(who.org.0.clone())])?;
    match rows.first().and_then(|row| row.first()) {
        Some(Value::Text(name)) => Ok(name.clone()),
        _ => Err(Fail::NotFound),
    }
}

fn rename(who: &Who, tx: &mut dyn Tx, body: &str) -> Result<String, Fail> {
    let name = body.trim();
    if name.is_empty() {
        return Err(Fail::Invalid("an org needs a name".to_owned()));
    }
    tx.run(
        RENAME_SQL,
        &[Value::Text(who.org.0.clone()), Value::Text(name.to_owned())],
    )?;
    Ok(name.to_owned())
}
