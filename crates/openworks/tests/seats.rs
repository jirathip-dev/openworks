mod memory;
mod table;

use memory::{ok, send};
use openworks::Openworks;
use openworks_core::{Fail, Row, Tx, Value};
use openworks_db::{jobs, seats, workers};
use table::{Trail, change, select, text};

#[derive(Clone, Default)]
struct Store {
    seats: Vec<Row>,
    days: Vec<Row>,
    trail: Trail,
}

const JOBS: [(&str, &str); 3] = [("j1", "open"), ("j2", "open"), ("jc", "cancelled")];

fn status(org: &Value, job: &Value) -> Option<&'static str> {
    JOBS.iter()
        .find(|(id, _)| *org == text("o1") && *job == text(id))
        .map(|(_, status)| *status)
}

impl Store {
    fn seat(&self, org: &Value, id: &Value) -> Option<&Row> {
        self.seats.iter().find(|seat| seat[0] == *org && seat[1] == *id)
    }

    fn joined(&self) -> impl Iterator<Item = (&Row, &Row)> {
        self.days
            .iter()
            .filter_map(|day| self.seat(&day[0], &day[1]).map(|seat| (day, seat)))
    }

    fn booked(&self, p: &[Value]) -> Result<Vec<Row>, Fail> {
        let (Value::Text(from), Value::Text(to)) = (&p[1], &p[2]) else {
            return Err(Fail::Store("booked takes two days".to_owned()));
        };
        let mut rows: Vec<Row> = self
            .joined()
            .filter(|(day, seat)| {
                day[0] == p[0] && status(&seat[0], &seat[2]) != Some("cancelled") && {
                    let Value::Text(at) = &day[2] else { return false };
                    from <= at && at <= to
                }
            })
            .map(|(day, seat)| vec![seat[1].clone(), seat[2].clone(), seat[4].clone(), day[2].clone()])
            .collect();
        rows.sort_by_key(|row| {
            (
                format!("{:?}", row[3]),
                row[2] == Value::Null,
                format!("{:?}{:?}", row[2], row[0]),
            )
        });
        Ok(rows)
    }
}

impl Tx for Store {
    fn run(&mut self, sql: &'static str, p: &[Value]) -> Result<Vec<Row>, Fail> {
        if let Some(rows) = self.trail.run(sql, p) {
            return Ok(rows);
        }
        Ok(match sql {
            jobs::GET => status(&p[0], &p[1]).map_or_else(Vec::new, |_| vec![vec![p[1].clone()]]),
            workers::GET => match &p[1] {
                Value::Text(id) if p[0] == text("o1") && ["ada", "bo"].contains(&id.as_str()) => {
                    vec![vec![p[1].clone()]]
                }
                _ => Vec::new(),
            },
            seats::LIST => {
                let mut rows = select(&self.seats, |seat| seat[0] == p[0] && seat[2] == p[1]);
                rows.sort_by_key(|row| format!("{:?}", row[4]));
                rows
            }
            seats::GET => select(&self.seats, |seat| seat[0] == p[0] && seat[1] == p[1]),
            seats::ADD => {
                self.seats.push(p.to_vec());
                Vec::new()
            }
            seats::SAVE => change(
                &mut self.seats,
                |seat| seat[..3] == p[..3],
                |seat| seat[3..].clone_from_slice(&p[3..]),
            ),
            seats::DAYS => select(&self.days, |day| day[0] == p[0] && day[1] == p[1])
                .into_iter()
                .map(|day| vec![day[1].clone()])
                .collect(),
            seats::HELD => self
                .joined()
                .filter(|(_, seat)| seat[0] == p[0] && seat[2] == p[1] && seat[4] == p[2] && seat[1] != p[3])
                .map(|(day, _)| vec![day[2].clone()])
                .collect(),
            seats::CLEAR => {
                self.days.retain(|day| day[0] != p[0] || day[1] != p[1]);
                Vec::new()
            }
            seats::ADD_DAY => {
                self.days.push(p.to_vec());
                Vec::new()
            }
            seats::BOOKED => self.booked(p)?,
            _ => Vec::new(),
        })
    }
}

type TestHost = memory::TestHost<Store>;

fn seat(id: &str, job: &str, title: &str, worker: &str, number: u8) -> String {
    format!(r#"{{"id":"{id}","job_id":"{job}","title":"{title}","worker_id":{worker},"number":{number},"ext":{{}}}}"#)
}

fn booking(seat: &str, job: &str, worker: &str, day: &str) -> String {
    format!(r#"{{"id":"{seat}","job_id":"{job}","worker_id":{worker},"day":"{day}","ext":{{}}}}"#)
}

#[test]
fn a_job_s_seats_carry_a_title_a_worker_and_a_number_on_the_job() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();

    let lead = ok(send(&openworks, "/seats/save", "owner", "\nj1\nLead\nada\n2"));
    let hand = ok(send(&openworks, "/seats/save", "owner", "\nj1\nHand\n\n1"));
    assert_eq!(
        ok(send(&openworks, "/seats/list", "viewer", "j1")),
        format!(
            "[{},{}]",
            seat(&hand, "j1", "Hand", "null", 1),
            seat(&lead, "j1", "Lead", r#""ada""#, 2)
        )
    );
    assert_eq!(
        ok(send(
            &openworks,
            "/seats/save",
            "owner",
            &format!("{hand}\nj1\nHand\nbo\n3")
        )),
        hand
    );
    assert!(ok(send(&openworks, "/seats/list", "viewer", "j1")).contains(&seat(&hand, "j1", "Hand", r#""bo""#, 3)));
    assert_eq!(ok(send(&openworks, "/seats/list", "viewer", "j2")), "[]");

    let taken = send(&openworks, "/seats/save", "owner", "\nj1\nSpare\n\n2");
    assert_eq!(
        (taken.status, taken.body.as_str()),
        (400, "invalid: another seat of the job is number 2")
    );
    assert_eq!(send(&openworks, "/seats/save", "owner", "\nj1\nSpare\n\n0").status, 400);
    assert_eq!(send(&openworks, "/seats/save", "owner", "\nj1\n\n\n").status, 400);
    assert_eq!(
        send(&openworks, "/seats/save", "owner", "\nj1\nSpare\nnobody\n").status,
        404
    );
    assert_eq!(
        send(&openworks, "/seats/save", "owner", "\nnojob\nSpare\n\n").status,
        404
    );
    assert_eq!(
        send(&openworks, "/seats/save", "owner", &format!("{lead}\nj2\nLead\n\n")).status,
        404
    );
    assert_eq!(send(&openworks, "/seats/save", "viewer", "\nj1\nSpare\n\n").status, 403);
    assert_eq!(send(&openworks, "/seats/list", "other", "j1").status, 404);
    assert_eq!(host.store().seats.len(), 2);
    assert_eq!(host.store().trail.audits, 3);
    assert_eq!(host.store().trail.events.len(), 3);
}

#[test]
fn a_worker_holds_one_seat_of_a_job_a_day() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    let lead = ok(send(&openworks, "/seats/save", "owner", "\nj1\nLead\nada\n1"));
    let hand = ok(send(&openworks, "/seats/save", "owner", "\nj1\nHand\nada\n2"));
    let held = "invalid: worker ada holds another seat of the job on 2026-01-06";

    let days = format!("{lead}\n2026-01-06\n2026-01-05\n2026-01-06");
    assert_eq!(ok(send(&openworks, "/seats/set-days", "owner", &days)), lead);
    assert_eq!(ok(send(&openworks, "/seats/set-days", "owner", &days)), lead);
    assert_eq!(host.store().days.len(), 2);
    let clash = send(
        &openworks,
        "/seats/set-days",
        "owner",
        &format!("{hand}\n2026-01-07\n2026-01-06"),
    );
    assert_eq!((clash.status, clash.body.as_str()), (400, held));
    ok(send(
        &openworks,
        "/seats/save",
        "owner",
        &format!("{hand}\nj1\nHand\n\n2"),
    ));
    ok(send(
        &openworks,
        "/seats/set-days",
        "owner",
        &format!("{hand}\n2026-01-06"),
    ));
    let moved_in = send(&openworks, "/seats/save", "owner", &format!("{hand}\nj1\nHand\nada\n2"));
    assert_eq!((moved_in.status, moved_in.body.as_str()), (400, held));
    ok(send(
        &openworks,
        "/seats/set-days",
        "owner",
        &format!("{lead}\n2026-01-05"),
    ));
    ok(send(
        &openworks,
        "/seats/save",
        "owner",
        &format!("{hand}\nj1\nHand\nada\n2"),
    ));

    assert_eq!(
        send(&openworks, "/seats/set-days", "owner", &format!("{lead}\n6 Jan")).status,
        400
    );
    assert_eq!(
        send(&openworks, "/seats/set-days", "owner", "nobody\n2026-01-06").status,
        404
    );
    assert_eq!(
        send(&openworks, "/seats/set-days", "other", &format!("{lead}\n2026-01-06")).status,
        404
    );
    assert_eq!(
        send(&openworks, "/seats/set-days", "viewer", &format!("{lead}\n2026-01-06")).status,
        403
    );
    assert_eq!(host.store().trail.events.len(), 8);
}

#[test]
fn bookings_show_who_works_where_across_jobs_not_cancelled() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    let lead = ok(send(&openworks, "/seats/save", "owner", "\nj1\nLead\nada\n1"));
    let hand = ok(send(&openworks, "/seats/save", "owner", "\nj1\nHand\n\n2"));
    let other = ok(send(&openworks, "/seats/save", "owner", "\nj2\nLead\nada\n1"));
    let gone = ok(send(&openworks, "/seats/save", "owner", "\njc\nLead\nada\n1"));
    for seat in [&lead, &hand, &other, &gone] {
        ok(send(
            &openworks,
            "/seats/set-days",
            "owner",
            &format!("{seat}\n2026-01-06\n2026-01-12"),
        ));
    }

    assert_eq!(
        ok(send(&openworks, "/seats/booked", "viewer", "2026-01-05\n2026-01-09")),
        format!(
            "[{},{},{}]",
            booking(&lead, "j1", r#""ada""#, "2026-01-06"),
            booking(&other, "j2", r#""ada""#, "2026-01-06"),
            booking(&hand, "j1", "null", "2026-01-06"),
        )
    );
    ok(send(&openworks, "/seats/set-days", "owner", &format!("{lead}\n")));
    assert!(!ok(send(&openworks, "/seats/booked", "viewer", "2026-01-05\n2026-01-12")).contains(&lead));
    assert_eq!(
        send(&openworks, "/seats/booked", "viewer", "2026-01-09\n2026-01-05").status,
        400
    );
    assert_eq!(
        ok(send(&openworks, "/seats/booked", "other", "2026-01-05\n2026-01-12")),
        "[]"
    );
}
