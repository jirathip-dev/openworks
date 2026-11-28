use std::collections::BTreeSet;

use openworks_core::{Declaration, Fail, Readiness, Record, Row, Tx, Value, Who, Window, Word};
use openworks_db::equipment::NEEDS;
use openworks_db::jobs::BOARD as BOARD_SQL;
use openworks_db::seats::{BOOKED, LIST as SEATS};
use openworks_db::tasks::LIST as TASKS;

use crate::clients::{lines, org};
use crate::jobs::{FIELDS, date, window};
use crate::projects::{plain, reaching, record, text};

/// Reads the planner's board from the body's `from` and `to` day lines: the jobs not cancelled whose planned window meets those days, by start, as records of kind `jobs` that also carry their live tasks, filled and open seats, worker clashes with other jobs, issues and whether they are ready.
pub const BOARD: Declaration = openworks_core::list!(
    "board",
    needs: Read,
    record: "jobs",
    queries: [BOARD_SQL, TASKS, SEATS, BOOKED, NEEDS],
    board,
);

fn board(who: &Who, tx: &mut dyn Tx, body: &str) -> Result<Vec<Record>, Fail> {
    let [from, to] = lines(body)?;
    let days = window(date(from)?, date(to)?)?;
    let (first, last) = (days.starts_on().to_string(), days.ends_on().to_string());
    let scope = reaching(who, Word::Read);
    let jobs = tx.run(BOARD_SQL, &[org(who), text(&first), text(&last), scope.clone()])?;
    let booked = tx.run(BOOKED, &[org(who), text(&first), text(&last), scope.clone()])?;
    let needs = tx.run(NEEDS, &[org(who), scope])?;
    jobs.into_iter()
        .map(|row| {
            let id = row.first().cloned().unwrap_or(Value::Null);
            let tasks = tx.run(TASKS, &[org(who), id.clone()])?;
            let seats = tx.run(SEATS, &[org(who), id.clone()])?;
            let filled = seats.iter().filter(|seat| at(seat, 3) != Value::Null).count();
            let readiness = Readiness {
                tasks: tasks.iter().filter(|task| at(task, 11) == Value::Null).count(),
                open_seats: seats.len() - filled,
                open_needs: needs
                    .iter()
                    .filter(|need| at(need, 1) == id && meets(need, days))
                    .count(),
                clashes: clashes(&booked, &id),
            };
            board_row(row, filled, readiness)
        })
        .collect()
}

fn board_row(mut row: Row, filled: usize, readiness: Readiness) -> Result<Record, Fail> {
    let issues = readiness.issues();
    let names: Vec<&str> = issues.iter().map(|issue| issue.name()).collect();
    row.extend([
        count(readiness.tasks),
        count(filled),
        count(readiness.open_seats),
        count(readiness.clashes),
        text(&names.join(",")),
        Value::Int(issues.is_empty().into()),
    ]);
    let fields = [
        &FIELDS[..],
        &["tasks", "crew", "open_seats", "clashes", "issues", "ready"],
    ]
    .concat();
    record(row, &fields)
}

fn clashes(booked: &[Row], job: &Value) -> usize {
    let mine: BTreeSet<(String, String)> = booked
        .iter()
        .filter(|day| at(day, 1) == *job && at(day, 2) != Value::Null)
        .map(|day| (plain(day.get(2)), plain(day.get(3))))
        .collect();
    let elsewhere: BTreeSet<(String, String)> = booked
        .iter()
        .filter(|day| at(day, 1) != *job)
        .map(|day| (plain(day.get(2)), plain(day.get(3))))
        .collect();
    mine.intersection(&elsewhere).count()
}

fn meets(need: &Row, days: Window) -> bool {
    plain(need.get(4)) <= days.ends_on().to_string() && plain(need.get(5)) >= days.starts_on().to_string()
}

fn at(row: &Row, index: usize) -> Value {
    row.get(index).cloned().unwrap_or(Value::Null)
}

fn count(n: usize) -> Value {
    Value::Int(i64::try_from(n).unwrap_or(i64::MAX))
}
