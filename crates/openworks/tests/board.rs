mod memory;
#[expect(dead_code, reason = "the board changes no row")]
mod table;

use memory::{ok, send};
use openworks::Openworks;
use openworks_core::{Fail, Row, Tx, Value};
use openworks_db::{equipment, jobs, seats, tasks};
use table::{Trail, select, text};

#[derive(Clone)]
struct Store {
    jobs: Vec<Row>,
    tasks: Vec<Row>,
    seats: Vec<Row>,
    days: Vec<Row>,
    needs: Vec<Row>,
    trail: Trail,
}

fn key(value: &Value) -> String {
    format!("{value:?}")
}

fn between(value: &Value, from: &Value, to: &Value) -> bool {
    key(from) <= key(value) && key(value) <= key(to)
}

fn row(values: &[&str]) -> Row {
    values
        .iter()
        .map(|value| if *value == "-" { Value::Null } else { text(value) })
        .collect()
}

fn job(id: &str, starts_on: &str, ends_on: &str) -> Row {
    let mut job = row(&[
        "o1", id, "p1", id, "open", "-", "-", "-", starts_on, ends_on, "-", "-", "", "-",
    ]);
    job[5] = Value::Int(1);
    job
}

impl Default for Store {
    fn default() -> Self {
        Self {
            jobs: vec![
                job("j1", "2026-01-05", "2026-01-09"),
                job("j2", "2026-01-07", "2026-01-12"),
                job("j3", "2026-02-02", "2026-02-06"),
            ],
            tasks: vec![
                row(&["o1", "t1", "j1", "", "Set out", "-", "0", "1", "-", "-", "1", "", "-"]),
                row(&["o1", "t2", "j1", "", "Old", "-", "0", "1", "-", "-", "2", "", "9"]),
                row(&["o1", "t3", "j2", "", "Old", "-", "0", "1", "-", "-", "1", "", "9"]),
            ],
            seats: vec![
                row(&["o1", "s1", "j1", "Lead", "ada", "-"]),
                row(&["o1", "s2", "j1", "Hand", "bo", "-"]),
                row(&["o1", "s3", "j2", "Lead", "ada", "-"]),
                row(&["o1", "s4", "j2", "Hand", "-", "-"]),
            ],
            days: vec![
                row(&["o1", "s1", "2026-01-06"]),
                row(&["o1", "s1", "2026-01-07"]),
                row(&["o1", "s2", "2026-01-07"]),
                row(&["o1", "s3", "2026-01-07"]),
            ],
            needs: vec![
                row(&["o1", "n1", "j2", "-", "lift", "2026-01-08", "2026-01-08", "-"]),
                row(&["o1", "n2", "j1", "-", "lift", "2026-01-20", "2026-01-21", "-"]),
                row(&["o1", "n3", "j1", "-", "lift", "2026-01-06", "2026-01-06", "b1"]),
            ],
            trail: Trail::default(),
        }
    }
}

impl Tx for Store {
    fn run(&mut self, sql: &'static str, p: &[Value]) -> Result<Vec<Row>, Fail> {
        if let Some(rows) = self.trail.run(sql, p) {
            return Ok(rows);
        }
        Ok(match sql {
            jobs::BOARD => select(&self.jobs, |job| {
                job[0] == p[0] && key(&job[8]) <= key(&p[2]) && key(&job[9]) >= key(&p[1])
            }),
            tasks::LIST => select(&self.tasks, |task| task[0] == p[0] && task[2] == p[1]),
            seats::LIST => select(&self.seats, |seat| seat[0] == p[0] && seat[2] == p[1]),
            seats::BOOKED => self
                .days
                .iter()
                .filter(|day| day[0] == p[0] && between(&day[2], &p[1], &p[2]))
                .filter_map(|day| {
                    self.seats
                        .iter()
                        .find(|seat| seat[..2] == day[..2])
                        .map(|seat| (day, seat))
                })
                .map(|(day, seat)| vec![seat[1].clone(), seat[2].clone(), seat[4].clone(), day[2].clone()])
                .collect(),
            equipment::NEEDS => select(&self.needs, |need| need[0] == p[0] && need[7] == Value::Null)
                .into_iter()
                .map(|need| need[..6].to_vec())
                .collect(),
            _ => Vec::new(),
        })
    }
}

type TestHost = memory::TestHost<Store>;

#[test]
fn the_board_counts_each_job_s_tasks_and_crew_and_computes_its_clashes_issues_and_readiness_for_the_days() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    let board = |body: &str| ok(send(&openworks, "/jobs/board", "viewer", body));

    let week = board("2026-01-05\n2026-01-09");
    let rows: Vec<&str> = week.split(r#"{"id":"#).skip(1).collect();
    assert_eq!(rows.len(), 2, "{week}");
    assert!(rows[0].starts_with(r#""j1""#), "{week}");
    assert!(
        rows[0].ends_with(r#","tasks":1,"crew":2,"open_seats":0,"clashes":1,"issues":"clash","ready":0,"ext":{}},"#),
        "{week}"
    );
    assert!(rows[1].starts_with(r#""j2""#), "{week}");
    let j2 = r#","tasks":0,"crew":1,"open_seats":1,"clashes":1,"issues":"no_tasks,open_seat,open_need,clash","ready":0,"ext":{}}]"#;
    assert!(rows[1].ends_with(j2), "{week}");

    let early = board("2026-01-05\n2026-01-06");
    assert!(
        early.starts_with(r#"[{"id":"j1""#) && early.matches(r#"{"id":"#).count() == 1,
        "{early}"
    );
    assert!(
        early.ends_with(r#","tasks":1,"crew":2,"open_seats":0,"clashes":0,"issues":"","ready":1,"ext":{}}]"#),
        "{early}"
    );
    assert_eq!(host.store().trail.audits, 0);
    assert_eq!(host.store().trail.events.len(), 0);
}
