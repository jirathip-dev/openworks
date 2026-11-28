use openworks_core::{Call, Declaration, Fail, Mount, Reach, Record, Tx, Value, Who};
use openworks_db::teams::GET as TEAM;
use openworks_db::workers::{
    ADD, GET as GET_SQL, LIST as LIST_SQL, ME as ME_SQL, SAVE as SAVE_SQL, SET_ACTIVE as SET_ACTIVE_SQL,
};

use crate::clients::{changed, found, lines, named, optional, org};
use crate::projects::{plain, records, text};

/// Lists the org's workers, as records of kind `workers`.
pub const LIST: Declaration = openworks_core::list!("list", needs: Read, record: "workers", queries: [LIST_SQL], list);

/// Reads the worker with the body's id, as a page of one record of kind `workers`.
pub const GET: Declaration = openworks_core::list!("get", needs: Read, record: "workers", queries: [GET_SQL], get);

/// Reads the worker the signed-in person is, as a page of one record of kind `workers`.
pub const ME: Declaration = openworks_core::list!("me", needs: Read, record: "workers", queries: [ME_SQL], me);

/// Saves a worker from a body of `id`, `name`, `title`, `team_id`, `company` (empty for own staff) and `person_id` (empty for no user) lines; an empty id adds one; answers its id.
pub const SAVE: Declaration =
    openworks_core::create!("save", needs: Write, queries: [TEAM, ME_SQL, ADD, SAVE_SQL], save);

/// Sets the worker with the body's id active or not, from a body of `id` and `active` (yes or no) lines.
pub const SET_ACTIVE: Declaration =
    openworks_core::write!("set-active", needs: Write, queries: [SET_ACTIVE_SQL], set_active);

const FIELDS: [&str; 6] = ["name", "title", "team_id", "company", "person_id", "active"];

fn list(who: &Who, tx: &mut dyn Tx, _: &str) -> Result<Vec<Record>, Fail> {
    records(tx.run(LIST_SQL, &[org(who)])?, &FIELDS)
}

fn get(who: &Who, tx: &mut dyn Tx, body: &str) -> Result<Vec<Record>, Fail> {
    found(records(tx.run(GET_SQL, &[org(who), text(body.trim())])?, &FIELDS)?)
}

fn me(who: &Who, tx: &mut dyn Tx, _: &str) -> Result<Vec<Record>, Fail> {
    found(records(tx.run(ME_SQL, &[org(who), text(&who.person.0)])?, &FIELDS)?)
}

fn save(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let [id, name, title, team, company, person] = lines(call.body)?;
    let team = optional(team);
    if team != Value::Null && tx.run(TEAM, &[org(call.who), team.clone()])?.is_empty() {
        return Err(Fail::NotFound);
    }
    let person = optional(person);
    if person != Value::Null {
        let taken = tx.run(ME_SQL, &[org(call.who), person.clone()])?;
        if taken.iter().any(|row| plain(row.first()) != id) {
            return Err(Fail::Invalid(format!(
                "person {} is another worker",
                plain(Some(&person))
            )));
        }
    }
    let adds = id.is_empty();
    let id = if adds { (call.fresh)().0 } else { id.to_owned() };
    let params = [
        org(call.who),
        text(&id),
        named(name)?,
        text(title),
        team,
        optional(company),
        person,
    ];
    if adds {
        tx.run(ADD, &params)?;
        Ok(id)
    } else {
        changed(tx.run(SAVE_SQL, &params)?)
    }
}

fn set_active(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let [id, active] = lines(call.body)?;
    let active = match active {
        "yes" => 1,
        "no" => 0,
        _ => return Err(Fail::Invalid("active is yes or no".to_owned())),
    };
    changed(tx.run(SET_ACTIVE_SQL, &[org(call.who), text(id), Value::Int(active)])?)
}

pub(crate) fn mount() -> Mount {
    Mount::new("/workers", "workers")
        .operation(&LIST, Reach::Org)
        .operation(&GET, Reach::Org)
        .operation(&ME, Reach::Org)
        .operation(&SAVE, Reach::Org)
        .operation(&SET_ACTIVE, Reach::Org)
}
