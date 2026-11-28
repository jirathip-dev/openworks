use openworks_core::{
    Call, Date, Declaration, Fail, JobStatus, JobStep, Record, Row, StageKind, Tx, Value, Who, Window, Word, WorkDays,
};
use openworks_db::jobs::{BOARD as BOARD_SQL, GET as GET_SQL, LIST as LIST_SQL, MOVE, PLAN, SAVE as SAVE_SQL};
use openworks_db::{projects, series, stages};

use crate::clients::{changed, lines, org};
use crate::projects::{first, plain, project, reaching, record, records, reopened, text};
use crate::{Period, Series};

const NUMBERS: Series = Series {
    key: "jobs",
    pattern: "J-{period}-{n}",
    period: Period::Year,
};

/// Lists the jobs of the project with the body's id, as records of kind `jobs`.
pub const LIST: Declaration =
    openworks_core::list!("list", needs: Read, record: "jobs", queries: [projects::GET, LIST_SQL], list);

/// Lists the planner's rows: the jobs not cancelled whose planned window meets the days from the body's `from` line to its `to` line, by start, as records of kind `jobs`.
pub const BOARD: Declaration = openworks_core::list!("board", needs: Read, record: "jobs", queries: [BOARD_SQL], board);

/// Reads the job with the body's id, as a page of one record of kind `jobs`.
pub const GET: Declaration = openworks_core::list!("get", needs: Read, record: "jobs", queries: [GET_SQL], get);

/// Saves a job from `id`, `at_site` (yes, or no for workshop work), `base_date`, `work_days` (ISO weekdays such as 1,2,3,4,5), `actual_start` and `remark` lines.
pub const SAVE: Declaration = openworks_core::write!("save", needs: Write, queries: [GET_SQL, SAVE_SQL], save);

/// Sets the planned window of a proposed or open job from `id`, `starts_on` and `ends_on` lines.
pub const RESCHEDULE: Declaration =
    openworks_core::write!("reschedule", needs: Write, queries: [GET_SQL, PLAN], reschedule);

/// Opens the proposed job with the body's id straight from the planner and answers its number.
pub const OPEN: Declaration =
    openworks_core::write!("open", needs: Write, queries: [GET_SQL, series::NEXT, MOVE], open_planned);

/// Finishes an open job from `id` and `actual_end` lines.
pub const FINISH: Declaration = openworks_core::write!("finish", needs: Write, queries: [GET_SQL, MOVE], finish);

/// Reopens the finished job with the body's id; a done project goes back to its company's first won stage.
pub const REOPEN: Declaration = openworks_core::write!(
    "reopen",
    needs: Write,
    queries: [GET_SQL, MOVE, projects::GET, projects::MOVE, stages::GET, stages::LIST],
    reopen,
);

/// Cancels the open job with the body's id.
pub const CANCEL: Declaration = openworks_core::write!("cancel", needs: Write, queries: [GET_SQL, MOVE], cancel);

pub(crate) const FIELDS: [&str; 12] = [
    "project_id",
    "number",
    "status",
    "at_site",
    "base_date",
    "work_days",
    "planned_start",
    "planned_end",
    "actual_start",
    "actual_end",
    "remark",
    "finished_at",
];

const PROJECT: usize = 1;
const NUMBER: usize = 2;
const STATUS: usize = 3;
const AT_SITE: usize = 4;
const PLANNED_START: usize = 7;
const ACTUAL_START: usize = 9;

fn list(who: &Who, tx: &mut dyn Tx, body: &str) -> Result<Vec<Record>, Fail> {
    let id = body.trim();
    project(who, tx, id, Word::Read)?;
    let rows = tx.run(LIST_SQL, &[org(who), text(id)])?;
    rows.into_iter().map(|row| record(row, &FIELDS)).collect()
}

fn board(who: &Who, tx: &mut dyn Tx, body: &str) -> Result<Vec<Record>, Fail> {
    let [from, to] = lines(body)?;
    let days = window(date(from)?, date(to)?)?;
    let params = [
        org(who),
        text(&days.starts_on().to_string()),
        text(&days.ends_on().to_string()),
        reaching(who, Word::Read),
    ];
    records(tx.run(BOARD_SQL, &params)?, &FIELDS)
}

fn get(who: &Who, tx: &mut dyn Tx, body: &str) -> Result<Vec<Record>, Fail> {
    Ok(vec![record(job(who, tx, body.trim(), Word::Read)?, &FIELDS)?])
}

fn save(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let who = call.who;
    let [id, at_site, base_date, work_days, actual_start, remark] = lines(call.body)?;
    let row = job(who, tx, id, Word::Write)?;
    let at_site = match at_site {
        "yes" | "" => 1,
        "no" => 0,
        _ => return Err(Fail::Invalid("at_site is yes or no".to_owned())),
    };
    let work_days = match work_days {
        "" => Value::Null,
        days => text(
            &WorkDays::parse(days)
                .ok_or_else(|| Fail::Invalid(format!("not ISO weekdays: {days}")))?
                .to_string(),
        ),
    };
    let actual_start = day(actual_start)?;
    if row.get(ACTUAL_START) != Some(&actual_start) {
        step(&row, JobStep::Record)?;
    }
    let params = [
        org(who),
        text(id),
        Value::Int(at_site),
        day(base_date)?,
        work_days,
        actual_start,
        text(remark),
    ];
    changed(tx.run(SAVE_SQL, &params)?)
}

fn reschedule(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let [id, starts_on, ends_on] = lines(call.body)?;
    let row = job(call.who, tx, id, Word::Write)?;
    step(&row, JobStep::Reschedule)?;
    let window = window(date(starts_on)?, date(ends_on)?)?;
    let params = [
        org(call.who),
        text(id),
        text(&window.starts_on().to_string()),
        text(&window.ends_on().to_string()),
    ];
    changed(tx.run(PLAN, &params)?)
}

fn open_planned(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let row = job(call.who, tx, call.body.trim(), Word::Write)?;
    open(call, tx, &row)
}

fn finish(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let [id, actual_end] = lines(call.body)?;
    let row = job(call.who, tx, id, Word::Write)?;
    let to = step(&row, JobStep::Finish)?;
    let Some(Value::Text(started)) = row.get(ACTUAL_START) else {
        return Err(Fail::Invalid(
            "a job records its actual start before it finishes".to_owned(),
        ));
    };
    let ends_on = window(date(started)?, date(actual_end)?)?.ends_on();
    mark(call, tx, &row, to, text(&ends_on.to_string()), Value::Int(call.at))
}

fn reopen(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let row = job(call.who, tx, call.body.trim(), Word::Write)?;
    let to = step(&row, JobStep::Reopen)?;
    let id = mark(call, tx, &row, to, Value::Null, Value::Null)?;
    reopened(call.who, tx, &plain(row.get(PROJECT)))?;
    Ok(id)
}

fn cancel(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let row = job(call.who, tx, call.body.trim(), Word::Write)?;
    let to = step(&row, JobStep::Cancel)?;
    mark(call, tx, &row, to, Value::Null, Value::Null)
}

pub(crate) fn follow(call: &Call, tx: &mut dyn Tx, project: &str, from: StageKind, to: StageKind) -> Result<(), Fail> {
    let jobs = tx.run(LIST_SQL, &[org(call.who), text(project)])?;
    for row in &jobs {
        let status = status(row)?;
        match (from, to, status) {
            (StageKind::Open, StageKind::Won, JobStatus::Proposed) => {
                open(call, tx, row)?;
            }
            (StageKind::Won, StageKind::Open, JobStatus::Open) => {
                mark(call, tx, row, step(row, JobStep::Unwin)?, Value::Null, Value::Null)?;
            }
            (_, StageKind::Done, JobStatus::Proposed | JobStatus::Open) => {
                return Err(Fail::Invalid(
                    "a project is done once every job is finished or cancelled".to_owned(),
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

fn open(call: &Call, tx: &mut dyn Tx, row: &Row) -> Result<String, Fail> {
    let to = step(row, JobStep::Open)?;
    if row.get(AT_SITE) != Some(&Value::Int(0)) && row.get(PLANNED_START) == Some(&Value::Null) {
        return Err(Fail::Invalid(format!(
            "job {} needs a planned window unless it is workshop work",
            plain(row.first())
        )));
    }
    let number = match row.get(NUMBER) {
        Some(Value::Text(number)) => number.clone(),
        _ => NUMBERS.next(call, tx)?,
    };
    let mut numbered = row.clone();
    if let Some(slot) = numbered.get_mut(NUMBER) {
        *slot = text(&number);
    }
    mark(call, tx, &numbered, to, Value::Null, Value::Null)?;
    Ok(number)
}

fn mark(
    call: &Call,
    tx: &mut dyn Tx,
    row: &Row,
    to: JobStatus,
    actual_end: Value,
    finished_at: Value,
) -> Result<String, Fail> {
    let number = row.get(NUMBER).cloned().unwrap_or(Value::Null);
    let params = [
        org(call.who),
        text(&plain(row.first())),
        text(to.name()),
        number,
        actual_end,
        finished_at,
    ];
    changed(tx.run(MOVE, &params)?)
}

fn step(row: &Row, step: JobStep) -> Result<JobStatus, Fail> {
    let from = status(row)?;
    from.after(step)
        .ok_or_else(|| Fail::Invalid(format!("a job that is {} cannot {}", from.name(), step.name())))
}

fn status(row: &Row) -> Result<JobStatus, Fail> {
    JobStatus::named(&plain(row.get(STATUS))).ok_or_else(|| Fail::Store("a job row holds no status".to_owned()))
}

pub(crate) fn job(who: &Who, tx: &mut dyn Tx, id: &str, word: Word) -> Result<Row, Fail> {
    first(tx.run(GET_SQL, &[org(who), text(id), reaching(who, word)])?)
}

pub(crate) fn window(starts_on: Date, ends_on: Date) -> Result<Window, Fail> {
    Window::new(starts_on, ends_on).ok_or_else(|| Fail::Invalid(format!("{ends_on} is before {starts_on}")))
}

pub(crate) fn date(day: &str) -> Result<Date, Fail> {
    Date::parse(day).ok_or_else(|| Fail::Invalid(format!("not a YYYY-MM-DD day: {day}")))
}

fn day(day: &str) -> Result<Value, Fail> {
    if day.is_empty() {
        Ok(Value::Null)
    } else {
        Ok(text(&date(day)?.to_string()))
    }
}
