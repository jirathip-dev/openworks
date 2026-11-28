use openworks_core::{Call, Declaration, Fail, Record, Tx, Who};
use openworks_db::teams::{ADD, LIST as LIST_SQL, SAVE as SAVE_SQL};

use crate::clients::{changed, lines, named, org};
use crate::projects::{records, text};

/// Lists the org's teams, as records of kind `teams`.
pub const LIST: Declaration = openworks_core::list!("list", needs: Read, record: "teams", queries: [LIST_SQL], list);

/// Saves a team from a body of `id` and `name` lines; an empty id adds one; answers its id.
pub const SAVE: Declaration = openworks_core::create!("save", needs: Write, queries: [ADD, SAVE_SQL], save);

fn list(who: &Who, tx: &mut dyn Tx, _: &str) -> Result<Vec<Record>, Fail> {
    records(tx.run(LIST_SQL, &[org(who)])?, &["name"])
}

fn save(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let [id, name] = lines(call.body)?;
    let name = named(name)?;
    if id.is_empty() {
        let id = (call.fresh)().0;
        tx.run(ADD, &[org(call.who), text(&id), name])?;
        return Ok(id);
    }
    changed(tx.run(SAVE_SQL, &[org(call.who), text(id), name])?)
}
