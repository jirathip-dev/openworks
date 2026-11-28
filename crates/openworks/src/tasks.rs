use openworks_core::{Call, Declaration, Fail, Hours, Record, Row, Tx, Value, Who, Word};
use openworks_db::jobs::GET as JOB;
use openworks_db::projects::GET as PROJECT;
use openworks_db::tasks::{
    ADD, ARCHIVE as ARCHIVE_SQL, BY_PROJECT as BY_PROJECT_SQL, GET as GET_SQL, LIST as LIST_SQL,
    RESTORE as RESTORE_SQL, SAVE as SAVE_SQL,
};

use crate::clients::{changed, named, optional, org};
use crate::projects::{first, plain, project, reaching, record, records, text};

/// Lists the tasks of the job with the body's id in order, archived ones too, as records of kind `tasks`.
pub const LIST: Declaration =
    openworks_core::list!("list", needs: Read, record: "tasks", queries: [JOB, LIST_SQL], list);

/// Lists the tasks of the project with the body's id, job by job in order, archived ones too, as records of kind `tasks`.
pub const BY_PROJECT: Declaration =
    openworks_core::list!("by-project", needs: Read, record: "tasks", queries: [PROJECT, BY_PROJECT_SQL], by_project);

/// Saves a job's tasks in order from a `job_id` line, then one line per task of tab-separated `id` (empty adds one), `code`, `name`, `color` (#rrggbb), `start_offset` and `days` (whole days), `planned_hours` (hundredths of an hour), `weight` (ten-thousandths), `order` (empty puts it last) and `remark`; answers their ids, a line each.
pub const SAVE: Declaration =
    openworks_core::create!("save", needs: Write, queries: [JOB, LIST_SQL, GET_SQL, ADD, SAVE_SQL], save);

/// Archives the task with the body's id unless hours are reported on it.
pub const ARCHIVE: Declaration =
    openworks_core::write!("archive", needs: Write, queries: [GET_SQL, ARCHIVE_SQL], archive);

/// Restores the archived task with the body's id.
pub const RESTORE: Declaration =
    openworks_core::write!("restore", needs: Write, queries: [GET_SQL, RESTORE_SQL], restore);

const FIELDS: [&str; 11] = [
    "job_id",
    "code",
    "name",
    "color",
    "start_offset",
    "days",
    "planned_hours",
    "weight",
    "order",
    "remark",
    "archived_at",
];

const JOB_ID: usize = 1;
const ORDER: usize = 9;

fn list(who: &Who, tx: &mut dyn Tx, body: &str) -> Result<Vec<Record>, Fail> {
    let id = body.trim();
    job(who, tx, id, Word::Read)?;
    let rows = tx.run(LIST_SQL, &[org(who), text(id)])?;
    rows.into_iter().map(|row| record(row, &FIELDS)).collect()
}

fn by_project(who: &Who, tx: &mut dyn Tx, body: &str) -> Result<Vec<Record>, Fail> {
    let id = body.trim();
    project(who, tx, id, Word::Read)?;
    records(tx.run(BY_PROJECT_SQL, &[org(who), text(id)])?, &FIELDS)
}

fn save(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let who = call.who;
    let mut body = call.body.lines();
    let job_id = body.next().unwrap_or("").trim();
    job(who, tx, job_id, Word::Write)?;
    let mut last = tx
        .run(LIST_SQL, &[org(who), text(job_id)])?
        .iter()
        .filter_map(|row| match row.get(ORDER) {
            Some(Value::Int(order)) => Some(*order),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    let mut saved = Vec::new();
    for line in body.filter(|line| !line.trim().is_empty()) {
        let fields: Vec<&str> = line.split('\t').map(str::trim).collect();
        let [id, code, name, color, offset, days, hours, weight, order, remark] = fields[..] else {
            return Err(Fail::Invalid("a task line is 10 tab-separated fields".to_owned()));
        };
        let order = match order {
            "" => last + 1,
            order => count("order", order)?,
        };
        last = last.max(order);
        let adds = id.is_empty();
        if !adds && plain(task(who, tx, id)?.get(JOB_ID)) != job_id {
            return Err(Fail::Invalid(format!("task {id} is not on job {job_id}")));
        }
        let id = if adds { (call.fresh)().0 } else { id.to_owned() };
        let offset = if offset.is_empty() {
            0
        } else {
            count("start_offset", offset)?
        };
        let params = [
            org(who),
            text(&id),
            text(job_id),
            text(code),
            named(name)?,
            colour(color)?,
            Value::Int(offset),
            lasting(days)?,
            planned(hours)?,
            optional_count("weight", weight)?,
            Value::Int(order),
            text(remark),
        ];
        if adds {
            tx.run(ADD, &params)?;
            saved.push(id);
        } else {
            saved.push(changed(tx.run(SAVE_SQL, &params)?)?);
        }
    }
    Ok(saved.join("\n"))
}

fn archive(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let id = call.body.trim();
    task(call.who, tx, id)?;
    // TODO(#19)
    let reported = Hours::ZERO;
    if !reported.lets_archive() {
        return Err(Fail::Invalid(format!("task {id} has hours reported on it")));
    }
    changed(tx.run(ARCHIVE_SQL, &[org(call.who), text(id), Value::Int(call.at)])?)
}

fn restore(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let id = call.body.trim();
    task(call.who, tx, id)?;
    changed(tx.run(RESTORE_SQL, &[org(call.who), text(id)])?)
}

fn job(who: &Who, tx: &mut dyn Tx, id: &str, word: Word) -> Result<Row, Fail> {
    first(tx.run(JOB, &[org(who), text(id), reaching(who, word)])?)
}

fn task(who: &Who, tx: &mut dyn Tx, id: &str) -> Result<Row, Fail> {
    first(tx.run(GET_SQL, &[org(who), text(id), reaching(who, Word::Write)])?)
}

pub(crate) fn count(field: &str, value: &str) -> Result<i64, Fail> {
    value
        .parse::<i64>()
        .ok()
        .filter(|count| *count >= 0)
        .ok_or_else(|| Fail::Invalid(format!("{field} is a whole number, not {value}")))
}

fn optional_count(field: &str, value: &str) -> Result<Value, Fail> {
    if value.is_empty() {
        Ok(Value::Null)
    } else {
        Ok(Value::Int(count(field, value)?))
    }
}

pub(crate) fn lasting(days: &str) -> Result<Value, Fail> {
    match days {
        "" => Ok(Value::Int(1)),
        days => match count("days", days)? {
            0 => Err(Fail::Invalid("a task lasts at least one day".to_owned())),
            days => Ok(Value::Int(days)),
        },
    }
}

pub(crate) fn planned(value: &str) -> Result<Value, Fail> {
    if value.is_empty() {
        return Ok(Value::Null);
    }
    value
        .parse()
        .ok()
        .and_then(Hours::hundredths)
        .map(|hours| Value::Int(hours.get()))
        .ok_or_else(|| Fail::Invalid(format!("planned_hours is whole hundredths of an hour, not {value}")))
}

fn colour(value: &str) -> Result<Value, Fail> {
    let hex = value.strip_prefix('#').unwrap_or("");
    if value.is_empty() || (hex.len() == 6 && hex.bytes().all(|b| b.is_ascii_hexdigit())) {
        Ok(optional(value))
    } else {
        Err(Fail::Invalid(format!("a colour is #rrggbb, not {value}")))
    }
}
