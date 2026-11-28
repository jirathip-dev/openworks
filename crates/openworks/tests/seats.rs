mod memory;
mod table;

use memory::{ok, send};
use openworks::{Openworks, Response};
use openworks_core::{Fail, Row, Tx, Value};
use openworks_db::{jobs, seats, workers};
use table::{Trail, change, select, text};

#[derive(Clone, Default)]
struct Store {
    seats: Vec<Row>,
    days: Vec<Row>,
    trail: Trail,
}

fn key(value: &Value) -> String {
    format!("{value:?}")
}

impl Store {
    fn joined(&self) -> impl Iterator<Item = (&Row, &Row)> {
        let seat = |day: &Row| self.seats.iter().find(|seat| seat[..2] == day[..2]);
        self.days
            .iter()
            .filter_map(move |day| seat(day).map(|seat| (day, seat)))
    }
}

impl Tx for Store {
    fn run(&mut self, sql: &'static str, p: &[Value]) -> Result<Vec<Row>, Fail> {
        if let Some(rows) = self.trail.run(sql, p) {
            return Ok(rows);
        }
        let known = |ids: &[&str]| p[0] == text("o1") && ids.iter().any(|id| p[1] == text(id));
        Ok(match sql {
            jobs::GET if known(&["j1", "j2", "jc"]) => vec![vec![p[1].clone()]],
            workers::GET if known(&["ada", "bo"]) => vec![vec![p[1].clone()]],
            seats::LIST => select(&self.seats, |seat| seat[0] == p[0] && seat[2] == p[1]),
            seats::GET => select(&self.seats, |seat| seat[..2] == p[..2]),
            seats::ADD => {
                self.seats.push(p.to_vec());
                Vec::new()
            }
            seats::SAVE => change(
                &mut self.seats,
                |seat| seat[..3] == p[..3],
                |seat| seat[3..].clone_from_slice(&p[3..]),
            ),
            seats::DAYS => select(&self.days, |day| day[..2] == p[..2])
                .into_iter()
                .map(|day| day[1..].to_vec())
                .collect(),
            seats::HELD => self
                .joined()
                .filter(|(_, seat)| seat[0] == p[0] && seat[2] == p[1] && seat[4] == p[2] && seat[1] != p[3])
                .map(|(day, _)| vec![day[2].clone()])
                .collect(),
            seats::CLEAR => {
                self.days.retain(|day| day[..2] != p[..2]);
                Vec::new()
            }
            seats::ADD_DAY => {
                self.days.push(p.to_vec());
                Vec::new()
            }
            seats::BOOKED => self
                .joined()
                .filter(|(day, seat)| day[0] == p[0] && seat[2] != text("jc"))
                .filter(|(day, _)| key(&p[1]) <= key(&day[2]) && key(&day[2]) <= key(&p[2]))
                .map(|(day, seat)| vec![seat[1].clone(), seat[2].clone(), seat[4].clone(), day[2].clone()])
                .collect(),
            _ => Vec::new(),
        })
    }
}

type TestHost = memory::TestHost<Store>;

fn seat(id: &str, title: &str, worker: &str, number: u8) -> String {
    format!(r#"{{"id":"{id}","job_id":"j1","title":"{title}","worker_id":{worker},"number":{number},"ext":{{}}}}"#)
}

fn booking(seat: &str, job: &str, worker: &str) -> String {
    format!(r#"{{"id":"{seat}","job_id":"{job}","worker_id":{worker},"day":"2026-01-06","ext":{{}}}}"#)
}

fn refused(post: impl Fn(&str, &str, &str) -> Response, cases: &[(&str, &str, &str, u16)]) {
    for (path, who, body, status) in cases {
        assert_eq!(post(path, who, body).status, *status, "{path} {body}");
    }
}

#[test]
fn a_job_s_seats_carry_a_title_a_worker_and_a_number_on_the_job() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    let post = |path: &str, who: &str, body: &str| send(&openworks, path, who, body);

    let hand = ok(post("/seats/save", "owner", "\nj1\nHand\n\n1"));
    let lead = ok(post("/seats/save", "owner", "\nj1\nLead\nada\n2"));
    let listed = format!(
        "[{},{}]",
        seat(&hand, "Hand", "null", 1),
        seat(&lead, "Lead", r#""ada""#, 2)
    );
    assert_eq!(ok(post("/seats/list", "viewer", "j1")), listed);
    let renumbered = format!("{hand}\nj1\nHand\nbo\n3");
    assert_eq!(ok(post("/seats/save", "owner", &renumbered)), hand);
    assert!(ok(post("/seats/list", "viewer", "j1")).contains(&seat(&hand, "Hand", r#""bo""#, 3)));

    let taken = post("/seats/save", "owner", "\nj1\nSpare\n\n2");
    assert_eq!(taken.body, "invalid: another seat of the job is number 2");
    let moved = format!("{lead}\nj2\nLead\n\n");
    refused(
        post,
        &[
            ("/seats/save", "owner", "\nj1\nSpare\n\n0", 400),
            ("/seats/save", "owner", "\nj1\n\n\n", 400),
            ("/seats/save", "owner", "\nj1\nSpare\nnobody\n", 404),
            ("/seats/save", "owner", "\nnojob\nSpare\n\n", 404),
            ("/seats/save", "owner", &moved, 404),
            ("/seats/save", "viewer", "\nj1\nSpare\n\n", 403),
            ("/seats/list", "other", "j1", 404),
        ],
    );
    assert_eq!(host.store().seats.len(), 2);
    assert_eq!(host.store().trail.audits, 3);
}

#[test]
fn a_worker_holds_one_seat_of_a_job_a_day() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    let post = |path: &str, who: &str, body: &str| send(&openworks, path, who, body);
    let lead = ok(post("/seats/save", "owner", "\nj1\nLead\nada\n1"));
    let hand = ok(post("/seats/save", "owner", "\nj1\nHand\nada\n2"));
    let held = "invalid: worker ada holds another seat of the job on 2026-01-06";

    let days = format!("{lead}\n2026-01-06\n2026-01-05\n2026-01-06");
    assert_eq!(ok(post("/seats/set-days", "owner", &days)), lead);
    assert_eq!(ok(post("/seats/set-days", "owner", &days)), lead);
    assert_eq!(host.store().days.len(), 2);
    let both = format!("{hand}\n2026-01-07\n2026-01-06");
    assert_eq!(post("/seats/set-days", "owner", &both).body, held);
    ok(post("/seats/save", "owner", &format!("{hand}\nj1\nHand\n\n2")));
    ok(post("/seats/set-days", "owner", &format!("{hand}\n2026-01-06")));
    let rejoin = format!("{hand}\nj1\nHand\nada\n2");
    assert_eq!(post("/seats/save", "owner", &rejoin).body, held);
    ok(post("/seats/set-days", "owner", &format!("{lead}\n2026-01-05")));
    assert_eq!(ok(post("/seats/save", "owner", &rejoin)), hand);

    let day = format!("{lead}\n2026-01-06");
    refused(
        post,
        &[
            ("/seats/set-days", "owner", &format!("{lead}\n6 Jan"), 400),
            ("/seats/set-days", "owner", "nobody\n2026-01-06", 404),
            ("/seats/set-days", "other", &day, 404),
            ("/seats/set-days", "viewer", &day, 403),
        ],
    );
    assert_eq!(host.store().trail.events.len(), 8);
}

#[test]
fn bookings_show_who_works_where_across_jobs_not_cancelled() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    let post = |path: &str, who: &str, body: &str| send(&openworks, path, who, body);
    let mut seats = Vec::new();
    for job in ["j1\nLead\nada", "j2\nLead\nada", "j1\nHand\n", "jc\nLead\nada"] {
        let seat = ok(post("/seats/save", "owner", &format!("\n{job}\n")));
        ok(post(
            "/seats/set-days",
            "owner",
            &format!("{seat}\n2026-01-06\n2026-01-12"),
        ));
        seats.push(seat);
    }

    let booked = [
        booking(&seats[0], "j1", r#""ada""#),
        booking(&seats[1], "j2", r#""ada""#),
        booking(&seats[2], "j1", "null"),
    ];
    let week = ok(post("/seats/booked", "viewer", "2026-01-05\n2026-01-09"));
    assert_eq!(week, format!("[{}]", booked.join(",")));
    ok(post("/seats/set-days", "owner", &format!("{}\n", seats[0])));
    let left = ok(post("/seats/booked", "viewer", "2026-01-05\n2026-01-12"));
    assert!(!left.contains(&format!(r#""{}""#, seats[0])), "{left}");
    assert_eq!(post("/seats/booked", "viewer", "2026-01-09\n2026-01-05").status, 400);
    assert_eq!(ok(post("/seats/booked", "other", "2026-01-05\n2026-01-12")), "[]");
}
