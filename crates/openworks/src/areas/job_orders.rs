use std::fmt::Write as _;

use openworks_core::{Call, Declaration, Fail, Mount, Reach, Record, Row, Tx, Value, Who, Word};
use openworks_db::approvals::REQUEST;
use openworks_db::job_orders::{
    ADD, ADD_LINE, CLEAR, DRAFT, GET as GET_SQL, ISSUE as ISSUE_SQL, LINES, LIST as LIST_SQL, SUPERSEDE,
};
use openworks_db::jobs;

use crate::approvals::JOB_ORDER;
use crate::clients::{changed, org};
use crate::jobs::job;
use crate::projects::{first, plain, record, text};

/// Lists the revisions of the job with the body's id, oldest first, as records of kind `job_orders`.
pub const LIST: Declaration = openworks_core::list!(
    "list",
    needs: Read,
    record: "job_orders",
    queries: [jobs::GET, LIST_SQL, LINES],
    list,
);

/// Reads the revision with the body's id, as a page of one record of kind `job_orders`.
pub const GET: Declaration = openworks_core::list!(
    "get",
    needs: Read,
    record: "job_orders",
    queries: [GET_SQL, jobs::GET, LINES],
    get,
);

/// Prints the revision with the body's id as plain text: its job, revision, status and numbered lines.
pub const PRINT: Declaration = openworks_core::read!("print", needs: Read, queries: [GET_SQL, jobs::GET, LINES], print);

/// Saves a job's draft revision from a body of the job's id, then one line each; with no draft it starts the next revision.
pub const SAVE: Declaration = openworks_core::write!(
    "save",
    needs: Write,
    queries: [jobs::GET, DRAFT, ADD, CLEAR, ADD_LINE],
    save,
);

/// Issues the draft revision with the body's id, supersedes the job's last issued one and asks for the `job_order` approval; answers the approval's id.
pub const ISSUE: Declaration = openworks_core::write!(
    "issue",
    needs: Write,
    queries: [GET_SQL, jobs::GET, SUPERSEDE, ISSUE_SQL, REQUEST],
    issue,
);

const FIELDS: [&str; 6] = ["job_id", "revision", "status", "issued_by", "issued_at", "lines"];

const JOB: usize = 1;
const REVISION: usize = 2;
const STATUS: usize = 3;
const ISSUED_BY: usize = 4;
const NUMBER: usize = 2;

fn list(who: &Who, tx: &mut dyn Tx, body: &str) -> Result<Vec<Record>, Fail> {
    let id = body.trim();
    job(who, tx, id, Word::Read)?;
    let rows = tx.run(LIST_SQL, &[org(who), text(id)])?;
    rows.into_iter().map(|row| with_lines(who, tx, row)).collect()
}

fn get(who: &Who, tx: &mut dyn Tx, body: &str) -> Result<Vec<Record>, Fail> {
    let row = order(who, tx, body.trim(), Word::Read)?;
    Ok(vec![with_lines(who, tx, row)?])
}

fn print(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let who = call.who;
    let row = order(who, tx, call.body.trim(), Word::Read)?;
    let job = job(who, tx, &plain(row.get(JOB)), Word::Read)?;
    let number = match job.get(NUMBER) {
        Some(Value::Text(number)) => number.clone(),
        _ => plain(job.first()),
    };
    let revision = match row.get(REVISION) {
        Some(Value::Int(revision)) => *revision,
        _ => return Err(Fail::Store("a job order row holds no revision".to_owned())),
    };
    let status = match plain(row.get(STATUS)).as_str() {
        "draft" => "draft".to_owned(),
        status => format!("{status}, issued by {}", plain(row.get(ISSUED_BY))),
    };
    let mut print = format!("Job order {number}, revision {revision}\n{status}\n");
    for (at, line) in (1..).zip(lines(who, tx, &row)?) {
        writeln!(print, "{at}. {line}").map_err(|_| Fail::Store("the print did not write".to_owned()))?;
    }
    Ok(print)
}

fn save(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let who = call.who;
    let (id, body) = call.body.split_once('\n').unwrap_or((call.body, ""));
    let id = id.trim();
    let steps: Vec<&str> = body.lines().map(str::trim).filter(|line| !line.is_empty()).collect();
    if steps.is_empty() {
        return Err(Fail::Invalid("a job order needs a line".to_owned()));
    }
    job(who, tx, id, Word::Write)?;
    let order = if let Ok(draft) = changed(tx.run(DRAFT, &[org(who), text(id)])?) {
        draft
    } else {
        let order = (call.fresh)().0;
        tx.run(ADD, &[org(who), text(&order), text(id)])?;
        order
    };
    tx.run(CLEAR, &[org(who), text(&order)])?;
    for (at, step) in (1..).zip(steps) {
        tx.run(ADD_LINE, &[org(who), text(&order), Value::Int(at), text(step)])?;
    }
    Ok(order)
}

fn issue(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let who = call.who;
    let id = call.body.trim();
    let row = order(who, tx, id, Word::Write)?;
    if plain(row.get(STATUS)) != "draft" {
        return Err(Fail::Invalid(format!("{id} is issued and does not change")));
    }
    let person = text(&who.person.0);
    tx.run(SUPERSEDE, &[org(who), text(&plain(row.get(JOB)))])?;
    changed(tx.run(ISSUE_SQL, &[org(who), text(id), person.clone(), Value::Int(call.at)])?)?;
    let approval = (call.fresh)().0;
    let params = [
        org(who),
        text(&approval),
        text(JOB_ORDER.key),
        text(id),
        person,
        Value::Int(call.at),
        text(""),
    ];
    changed(tx.run(REQUEST, &params)?)
}

fn order(who: &Who, tx: &mut dyn Tx, id: &str, word: Word) -> Result<Row, Fail> {
    let row = first(tx.run(GET_SQL, &[org(who), text(id)])?)?;
    job(who, tx, &plain(row.get(JOB)), word)?;
    Ok(row)
}

fn lines(who: &Who, tx: &mut dyn Tx, row: &Row) -> Result<Vec<String>, Fail> {
    let rows = tx.run(LINES, &[org(who), text(&plain(row.first()))])?;
    Ok(rows.iter().map(|line| plain(line.first())).collect())
}

fn with_lines(who: &Who, tx: &mut dyn Tx, mut row: Row) -> Result<Record, Fail> {
    let lines = lines(who, tx, &row)?.join("\n");
    row.push(text(&lines));
    record(row, &FIELDS)
}

pub(crate) fn mount() -> Mount {
    Mount::new("/job-orders", "job_orders")
        .operation(&LIST, Reach::Org)
        .operation(&GET, Reach::Org)
        .operation(&PRINT, Reach::Org)
        .operation(&SAVE, Reach::Org)
        .operation(&ISSUE, Reach::Org)
}
