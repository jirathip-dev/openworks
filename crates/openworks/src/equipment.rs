use openworks_core::{Call, Declaration, Fail, Record, Tx, Value, Who, Window, Word};
use openworks_db::equipment::{
    ADD, BOOK, BOOKING, CANCEL as CANCEL_SQL, CLASHES, FILL, GET_NEED, LIST as LIST_SQL, LOCK, NEED as NEED_SQL,
    NEEDS as NEEDS_SQL, RELEASE, SAVE as SAVE_SQL, TIMELINE as TIMELINE_SQL,
};
use openworks_db::{jobs::GET as JOB, tasks::GET as TASK};

use crate::clients::{changed, lines, named, optional, org};
use crate::jobs::{date, job, window};
use crate::projects::{first, plain, reaching, records, text};

/// Lists every bookable thing by tag, as records of kind `equipment`.
pub const LIST: Declaration =
    openworks_core::list!("list", needs: Read, record: "equipment", queries: [LIST_SQL], list);

/// Saves a bookable thing from `id` (empty adds one), `tag`, `name`, `kind`, `place`, `holder_job_id` and `status` (`in_service`, the default, or `out_of_service`) lines; answers its id.
pub const SAVE: Declaration = openworks_core::create!("save", needs: Write, queries: [JOB, ADD, SAVE_SQL], save);

/// Books a thing from `equipment_id`, `job_id`, `task_id` (empty books the whole job), `starts_on` and `ends_on` lines unless a live booking of it meets those days; answers the booking's id.
pub const BOOK_THING: Declaration =
    openworks_core::create!("book", needs: Write, queries: [JOB, TASK, LOCK, CLASHES, BOOK], book);

/// Fills the open need on the body's `need_id` line with the thing of its kind on the `equipment_id` line, booked for the need's job, task and days; answers the booking's id.
pub const FILL_NEED: Declaration =
    openworks_core::create!("fill", needs: Write, queries: [GET_NEED, JOB, TASK, LOCK, CLASHES, BOOK, FILL], fill);

/// Cancels the live booking with the body's id; a need it filled is open again.
pub const CANCEL: Declaration =
    openworks_core::write!("cancel", needs: Write, queries: [BOOKING, JOB, CANCEL_SQL, RELEASE], cancel);

/// Adds a need of a kind of thing from `job_id`, `task_id` (empty for the whole job), `kind`, `starts_on` and `ends_on` lines; answers its id.
pub const NEED: Declaration = openworks_core::create!("need", needs: Write, queries: [JOB, TASK, NEED_SQL], need);

/// Lists the needs no booking fills yet, by first day, as records of kind `equipment_needs`.
pub const NEEDS: Declaration =
    openworks_core::list!("needs", needs: Read, record: "equipment_needs", queries: [NEEDS_SQL], needs);

/// Lists the live bookings that meet the days from the body's `from` line to its `to` line, thing by thing by tag, as records of kind `equipment_bookings`.
pub const TIMELINE: Declaration = openworks_core::list!(
    "timeline",
    needs: Read,
    record: "equipment_bookings",
    queries: [TIMELINE_SQL],
    timeline,
);

const FIELDS: [&str; 6] = ["tag", "name", "kind", "place", "holder_job_id", "status"];

const BOOKED: [&str; 5] = ["equipment_id", "job_id", "task_id", "starts_on", "ends_on"];

const NEEDED: [&str; 5] = ["job_id", "task_id", "kind", "starts_on", "ends_on"];

const TAG: usize = 1;
const KIND: usize = 3;
const STATUS: usize = 6;

fn list(who: &Who, tx: &mut dyn Tx, _: &str) -> Result<Vec<Record>, Fail> {
    records(tx.run(LIST_SQL, &[org(who)])?, &FIELDS)
}

fn needs(who: &Who, tx: &mut dyn Tx, _: &str) -> Result<Vec<Record>, Fail> {
    records(tx.run(NEEDS_SQL, &[org(who), reaching(who, Word::Read)])?, &NEEDED)
}

fn timeline(who: &Who, tx: &mut dyn Tx, body: &str) -> Result<Vec<Record>, Fail> {
    let [from, to] = lines(body)?;
    let days = window(date(from)?, date(to)?)?;
    let params = [
        org(who),
        text(&days.starts_on().to_string()),
        text(&days.ends_on().to_string()),
        reaching(who, Word::Read),
    ];
    records(tx.run(TIMELINE_SQL, &params)?, &BOOKED)
}

fn save(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let who = call.who;
    let [id, tag, name, kind, place, holder, status] = lines(call.body)?;
    if tag.is_empty() {
        return Err(Fail::Invalid("a tag is needed".to_owned()));
    }
    if !holder.is_empty() {
        job(who, tx, holder, Word::Write)?;
    }
    let status = match status {
        "" | "in_service" => "in_service",
        "out_of_service" => "out_of_service",
        _ => return Err(Fail::Invalid("status is in_service or out_of_service".to_owned())),
    };
    let adds = id.is_empty();
    let id = if adds { (call.fresh)().0 } else { id.to_owned() };
    let params = [
        org(who),
        text(&id),
        text(tag),
        named(name)?,
        text(kind),
        text(place),
        optional(holder),
        text(status),
    ];
    if adds {
        tx.run(ADD, &params)?;
        Ok(id)
    } else {
        changed(tx.run(SAVE_SQL, &params)?)
    }
}

fn book(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let [equipment, job_id, task, starts_on, ends_on] = lines(call.body)?;
    let days = window(date(starts_on)?, date(ends_on)?)?;
    let at = For::new(call.who, tx, job_id, task)?;
    booked(call, tx, equipment, &at, days, None)
}

fn fill(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let [need_id, equipment] = lines(call.body)?;
    let need = first(tx.run(GET_NEED, &[org(call.who), text(need_id)])?)?;
    if need.get(6) != Some(&Value::Null) {
        return Err(Fail::Invalid(format!("need {need_id} is booked")));
    }
    let at = For::new(call.who, tx, &plain(need.get(1)), &plain(need.get(2)))?;
    let days = window(date(&plain(need.get(4)))?, date(&plain(need.get(5)))?)?;
    let booking = booked(call, tx, equipment, &at, days, need.get(KIND))?;
    changed(tx.run(FILL, &[org(call.who), text(need_id), text(&booking)])?)?;
    Ok(booking)
}

fn cancel(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let id = call.body.trim();
    let booking = first(tx.run(BOOKING, &[org(call.who), text(id)])?)?;
    job(call.who, tx, &plain(booking.get(2)), Word::Write)?;
    let cancelled = changed(tx.run(CANCEL_SQL, &[org(call.who), text(id)])?)
        .map_err(|_| Fail::Invalid(format!("booking {id} is cancelled")))?;
    tx.run(RELEASE, &[org(call.who), text(id)])?;
    Ok(cancelled)
}

fn need(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let [job_id, task, kind, starts_on, ends_on] = lines(call.body)?;
    let at = For::new(call.who, tx, job_id, task)?;
    if kind.is_empty() {
        return Err(Fail::Invalid("a need names a kind".to_owned()));
    }
    let days = window(date(starts_on)?, date(ends_on)?)?;
    let id = (call.fresh)().0;
    let params = [
        org(call.who),
        text(&id),
        text(&at.job),
        at.task,
        text(kind),
        text(&days.starts_on().to_string()),
        text(&days.ends_on().to_string()),
    ];
    tx.run(NEED_SQL, &params)?;
    Ok(id)
}

struct For {
    job: String,
    task: Value,
}

impl For {
    fn new(who: &Who, tx: &mut dyn Tx, job_id: &str, task: &str) -> Result<Self, Fail> {
        job(who, tx, job_id, Word::Write)?;
        if !task.is_empty() {
            let row = first(tx.run(TASK, &[org(who), text(task), reaching(who, Word::Write)])?)?;
            if plain(row.get(1)) != job_id {
                return Err(Fail::Invalid(format!("task {task} is not on job {job_id}")));
            }
        }
        Ok(Self {
            job: job_id.to_owned(),
            task: optional(task),
        })
    }
}

fn booked(
    call: &Call,
    tx: &mut dyn Tx,
    equipment: &str,
    at: &For,
    days: Window,
    kind: Option<&Value>,
) -> Result<String, Fail> {
    let who = call.who;
    let thing = first(tx.run(LOCK, &[org(who), text(equipment)])?)?;
    let tag = plain(thing.get(TAG));
    if thing.get(STATUS) != Some(&text("in_service")) {
        return Err(Fail::Invalid(format!("{tag} is out of service")));
    }
    if kind.is_some_and(|kind| thing.get(KIND) != Some(kind)) {
        return Err(Fail::Invalid(format!("{tag} is not of the kind the need names")));
    }
    let (starts_on, ends_on) = (text(&days.starts_on().to_string()), text(&days.ends_on().to_string()));
    let clashes = tx.run(
        CLASHES,
        &[org(who), text(equipment), starts_on.clone(), ends_on.clone()],
    )?;
    if let Some(clash) = clashes.first() {
        return Err(Fail::Invalid(format!(
            "{tag} is booked on those days by {}",
            plain(clash.first())
        )));
    }
    let id = (call.fresh)().0;
    let params = [
        org(who),
        text(&id),
        text(equipment),
        text(&at.job),
        at.task.clone(),
        starts_on,
        ends_on,
    ];
    tx.run(BOOK, &params)?;
    Ok(id)
}
