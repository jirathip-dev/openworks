use std::collections::BTreeSet;

use openworks_core::{Call, Declaration, Fail, Record, Tx, Value, Who, Word};
use openworks_db::jobs::GET as JOB;
use openworks_db::seats::{
    ADD, ADD_DAY, BOOKED as BOOKED_SQL, CLEAR, DAYS, GET, HELD, LIST as LIST_SQL, SAVE as SAVE_SQL,
};
use openworks_db::workers::GET as WORKER;

use crate::clients::{changed, lines, optional, org};
use crate::jobs::{date, job, of_job, window};
use crate::projects::{first, plain, reaching, records, text};
use crate::tasks::count;

/// Lists the seats of the job with the body's id by number, as records of kind `seats`.
pub const LIST: Declaration =
    openworks_core::list!("list", needs: Read, record: "seats", queries: [JOB, LIST_SQL], list);

/// Saves a seat from `id` (empty adds one), `job_id`, `title`, `worker_id` (empty for none) and `number` (empty for none) lines; answers its id.
pub const SAVE: Declaration =
    openworks_core::create!("save", needs: Write, queries: [JOB, LIST_SQL, WORKER, DAYS, HELD, ADD, SAVE_SQL], save);

/// Sets the days the seat works from a body of its `id` line, then one YYYY-MM-DD day a line; answers its id.
pub const SET_DAYS: Declaration =
    openworks_core::write!("set-days", needs: Write, queries: [GET, JOB, HELD, CLEAR, ADD_DAY], set_days);

/// Lists who works where from the body's `from` day to its `to` day across jobs not cancelled, by day, as records of kind `seat_days`.
pub const BOOKED: Declaration =
    openworks_core::list!("booked", needs: Read, record: "seat_days", queries: [BOOKED_SQL], booked);

const FIELDS: [&str; 4] = ["job_id", "title", "worker_id", "number"];

fn list(who: &Who, tx: &mut dyn Tx, body: &str) -> Result<Vec<Record>, Fail> {
    of_job(who, tx, body, LIST_SQL, &FIELDS)
}

fn save(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let who = call.who;
    let [id, job_id, title, worker, number] = lines(call.body)?;
    job(who, tx, job_id, Word::Write)?;
    if title.is_empty() {
        return Err(Fail::Invalid("a seat needs a title".to_owned()));
    }
    let numbered = match number {
        "" => Value::Null,
        number => match count("number", number)? {
            0 => return Err(Fail::Invalid("a seat's number is at least 1".to_owned())),
            number => Value::Int(number),
        },
    };
    let seats = tx.run(LIST_SQL, &[org(who), text(job_id)])?;
    if !id.is_empty() && !seats.iter().any(|seat| plain(seat.first()) == id) {
        return Err(Fail::NotFound);
    }
    if numbered != Value::Null
        && seats
            .iter()
            .any(|seat| seat.get(4) == Some(&numbered) && plain(seat.first()) != id)
    {
        return Err(Fail::Invalid(format!("another seat of the job is number {number}")));
    }
    let worker = optional(worker);
    if worker != Value::Null {
        first(tx.run(WORKER, &[org(who), worker.clone()])?)?;
        let days = tx.run(DAYS, &[org(who), text(id)])?;
        held(who, tx, job_id, &worker, id, days.iter().map(|day| plain(day.first())))?;
    }
    let adds = id.is_empty();
    let id = if adds { (call.fresh)().0 } else { id.to_owned() };
    let params = [org(who), text(&id), text(job_id), text(title), worker, numbered];
    if adds {
        tx.run(ADD, &params)?;
        Ok(id)
    } else {
        changed(tx.run(SAVE_SQL, &params)?)
    }
}

fn set_days(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let who = call.who;
    let mut body = call.body.lines();
    let id = body.next().unwrap_or("").trim();
    let seat = first(tx.run(GET, &[org(who), text(id)])?)?;
    let job_id = plain(seat.get(1));
    job(who, tx, &job_id, Word::Write)?;
    let mut days = BTreeSet::new();
    for day in body.map(str::trim).filter(|day| !day.is_empty()) {
        days.insert(date(day)?.to_string());
    }
    let worker = seat.get(3).cloned().unwrap_or(Value::Null);
    if worker != Value::Null {
        held(who, tx, &job_id, &worker, id, days.iter().cloned())?;
    }
    tx.run(CLEAR, &[org(who), text(id)])?;
    for day in days {
        tx.run(ADD_DAY, &[org(who), text(id), text(&day)])?;
    }
    Ok(id.to_owned())
}

fn held(
    who: &Who,
    tx: &mut dyn Tx,
    job_id: &str,
    worker: &Value,
    seat: &str,
    days: impl Iterator<Item = String>,
) -> Result<(), Fail> {
    let taken: BTreeSet<String> = tx
        .run(HELD, &[org(who), text(job_id), worker.clone(), text(seat)])?
        .iter()
        .map(|row| plain(row.first()))
        .collect();
    match days.into_iter().find(|day| taken.contains(day)) {
        Some(day) => Err(Fail::Invalid(format!(
            "worker {} holds another seat of the job on {day}",
            plain(Some(worker))
        ))),
        None => Ok(()),
    }
}

fn booked(who: &Who, tx: &mut dyn Tx, body: &str) -> Result<Vec<Record>, Fail> {
    let [from, to] = lines(body)?;
    let days = window(date(from)?, date(to)?)?;
    let params = [
        org(who),
        text(&days.starts_on().to_string()),
        text(&days.ends_on().to_string()),
        reaching(who, Word::Read),
    ];
    records(tx.run(BOOKED_SQL, &params)?, &["job_id", "worker_id", "day"])
}
