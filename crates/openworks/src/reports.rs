use std::collections::BTreeSet;

use openworks_core::{Call, Declaration, Fail, Record, Row, Tx, Value, Who, Word};
use openworks_db::reports::{
    ADD, ADD_HOURS, ADD_PROGRESS, BY_DAY, CLEAR_HOURS, CLEAR_PROGRESS, EDIT, FIND, GET, HOURS as HOURS_SQL, KIND,
    LIST as LIST_SQL, PROGRESS as PROGRESS_SQL,
};
use openworks_db::{jobs, series, tasks, workers};

use crate::clients::{changed, lines, org};
use crate::jobs::{date, job, of_job};
use crate::projects::{first, plain, records, text};
use crate::tasks::count;
use crate::{Period, Series};

const NUMBERS: Series = Series {
    key: "daily_reports",
    pattern: "R-{period}-{n}",
    period: Period::Year,
};

/// Lists the reports of the job with the body's id by day, as records of kind `daily_reports`.
pub const LIST: Declaration =
    openworks_core::list!("list", needs: Read, record: "daily_reports", queries: [jobs::GET, LIST_SQL], list);

/// Starts the report of an open job on a day from `job_id` and `day` lines, numbering it, or finds the one there is; answers its id.
pub const SAVE: Declaration =
    openworks_core::create!("save", needs: Write, queries: [jobs::GET, FIND, series::NEXT, ADD], save);

/// Lists the hours of the report with the body's id, as records of kind `report_hours`.
pub const HOURS: Declaration =
    openworks_core::list!("hours", needs: Read, record: "report_hours", queries: [GET, jobs::GET, HOURS_SQL], hours);

/// Sets a report's hours from its `id` line, then one line each of tab-separated `worker_id`, `task_id`, `kind` and `hours` (hundredths of an hour); answers its id.
pub const SET_HOURS: Declaration = openworks_core::write!(
    "set-hours",
    needs: Write,
    queries: [GET, jobs::GET, workers::GET, tasks::GET, KIND, CLEAR_HOURS, ADD_HOURS, EDIT],
    set_hours,
);

/// Lists the declared progress of the report with the body's id, as records of kind `report_progress`.
pub const PROGRESS: Declaration = openworks_core::list!(
    "progress",
    needs: Read,
    record: "report_progress",
    queries: [GET, jobs::GET, PROGRESS_SQL],
    progress,
);

/// Sets a report's declared progress from its `id` line, then one line each of tab-separated `task_id` and `percent`; answers its id.
pub const SET_PROGRESS: Declaration = openworks_core::write!(
    "set-progress",
    needs: Write,
    queries: [GET, jobs::GET, tasks::GET, CLEAR_PROGRESS, ADD_PROGRESS, EDIT],
    set_progress,
);

/// Sums the hours of the job with the body's id by day and kind, voided reports left out, as records of kind `report_hours`.
pub const HOURS_BY_DAY: Declaration = openworks_core::list!(
    "hours-by-day",
    needs: Read,
    record: "report_hours",
    queries: [jobs::GET, BY_DAY],
    hours_by_day,
);

const FIELDS: [&str; 5] = ["job_id", "day", "number", "status", "version"];

const JOB: usize = 1;
const STATUS: usize = 4;
const JOB_STATUS: usize = 3;

fn list(who: &Who, tx: &mut dyn Tx, body: &str) -> Result<Vec<Record>, Fail> {
    of_job(who, tx, body, LIST_SQL, &FIELDS)
}

fn save(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let who = call.who;
    let [job_id, day] = lines(call.body)?;
    let row = job(who, tx, job_id, Word::Write)?;
    if plain(row.get(JOB_STATUS)) != "open" {
        return Err(Fail::Invalid(format!("job {job_id} is not open")));
    }
    let day = date(day)?.to_string();
    if let Ok(id) = changed(tx.run(FIND, &[org(who), text(job_id), text(&day)])?) {
        return Ok(id);
    }
    let number = NUMBERS.next(call, tx)?;
    let id = (call.fresh)().0;
    tx.run(ADD, &[org(who), text(&id), text(job_id), text(&day), text(&number)])?;
    Ok(id)
}

fn hours(who: &Who, tx: &mut dyn Tx, body: &str) -> Result<Vec<Record>, Fail> {
    let id = body.trim();
    report(who, tx, id, Word::Read)?;
    records(tx.run(HOURS_SQL, &[org(who), text(id)])?, &["task_id", "kind", "hours"])
}

fn set_hours(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let who = call.who;
    let (id, body) = call.body.split_once('\n').unwrap_or((call.body, ""));
    let id = id.trim();
    let job_id = editable(who, tx, id)?;
    let mut seen = BTreeSet::new();
    let mut rows = Vec::new();
    for line in body.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let [worker, task, kind, hours] = fields(line)?;
        if !seen.insert((worker, task, kind)) {
            return Err(Fail::Invalid(format!("{worker} has two lines of {kind} on {task}")));
        }
        first(tx.run(workers::GET, &[org(who), text(worker)])?)?;
        on_job(who, tx, task, &job_id)?;
        first(tx.run(KIND, &[org(who), text(kind)])?)?;
        rows.push([text(worker), text(task), text(kind), reported(hours)?]);
    }
    tx.run(CLEAR_HOURS, &[org(who), text(id)])?;
    for [worker, task, kind, hours] in rows {
        tx.run(ADD_HOURS, &[org(who), text(id), worker, task, kind, hours])?;
    }
    changed(tx.run(EDIT, &[org(who), text(id)])?)
}

fn progress(who: &Who, tx: &mut dyn Tx, body: &str) -> Result<Vec<Record>, Fail> {
    let id = body.trim();
    report(who, tx, id, Word::Read)?;
    records(tx.run(PROGRESS_SQL, &[org(who), text(id)])?, &["percent"])
}

fn set_progress(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let who = call.who;
    let (id, body) = call.body.split_once('\n').unwrap_or((call.body, ""));
    let id = id.trim();
    let job_id = editable(who, tx, id)?;
    let mut seen = BTreeSet::new();
    let mut rows = Vec::new();
    for line in body.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let [task, percent] = fields(line)?;
        if !seen.insert(task) {
            return Err(Fail::Invalid(format!("{task} has two lines of progress")));
        }
        on_job(who, tx, task, &job_id)?;
        let percent = count("percent", percent)?;
        if percent > 100 {
            return Err(Fail::Invalid(format!("percent is at most 100, not {percent}")));
        }
        rows.push((text(task), Value::Int(percent)));
    }
    tx.run(CLEAR_PROGRESS, &[org(who), text(id)])?;
    for (task, percent) in rows {
        tx.run(ADD_PROGRESS, &[org(who), text(id), task, percent])?;
    }
    changed(tx.run(EDIT, &[org(who), text(id)])?)
}

fn hours_by_day(who: &Who, tx: &mut dyn Tx, body: &str) -> Result<Vec<Record>, Fail> {
    of_job(who, tx, body, BY_DAY, &["day", "kind", "hours"])
}

fn report(who: &Who, tx: &mut dyn Tx, id: &str, word: Word) -> Result<Row, Fail> {
    let row = first(tx.run(GET, &[org(who), text(id)])?)?;
    job(who, tx, &plain(row.get(JOB)), word)?;
    Ok(row)
}

fn editable(who: &Who, tx: &mut dyn Tx, id: &str) -> Result<String, Fail> {
    let row = report(who, tx, id, Word::Write)?;
    match plain(row.get(STATUS)).as_str() {
        "draft" | "submitted" => Ok(plain(row.get(JOB))),
        status => Err(Fail::Invalid(format!("report {id} is {status} and does not change"))),
    }
}

fn on_job(who: &Who, tx: &mut dyn Tx, task: &str, job_id: &str) -> Result<(), Fail> {
    let row = crate::tasks::task(who, tx, task)?;
    if plain(row.get(crate::tasks::JOB_ID)) == job_id {
        Ok(())
    } else {
        Err(Fail::Invalid(format!("task {task} is not on job {job_id}")))
    }
}

fn fields<const N: usize>(line: &str) -> Result<[&str; N], Fail> {
    let fields: Vec<&str> = line.split('\t').map(str::trim).collect();
    fields
        .try_into()
        .map_err(|_| Fail::Invalid(format!("a line is {N} tab-separated fields: {line}")))
}

fn reported(hours: &str) -> Result<Value, Fail> {
    match count("hours", hours)? {
        0 => Err(Fail::Invalid("hours are more than none".to_owned())),
        hours => Ok(Value::Int(hours)),
    }
}
