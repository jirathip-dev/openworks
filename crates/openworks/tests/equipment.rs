mod memory;
mod trail;

use memory::{ok, send};
use openworks::Openworks;
use openworks_core::{Fail, Row, Tx, Value};
use openworks_db::{equipment, tasks};
use trail::{Trail, job_row, text};

#[derive(Clone, Default)]
struct Store {
    things: Vec<Row>,
    bookings: Vec<Row>,
    needs: Vec<Row>,
    trail: Trail,
}

fn task_row(id: &Value) -> Vec<Row> {
    let job = match id {
        Value::Text(id) if id == "t1" => "j1",
        Value::Text(id) if id == "t2" => "j2",
        _ => return Vec::new(),
    };
    let mut row = vec![id.clone(), text(job)];
    row.resize(12, Value::Null);
    vec![row]
}

fn keyed(table: &[Row], p: &[Value]) -> Vec<Row> {
    table
        .iter()
        .filter(|row| row[..2] == p[..2])
        .map(|row| row[1..].to_vec())
        .collect()
}

fn meets(row: &Row, from: &Value, to: &Value) -> bool {
    let day = |value: &Value| match value {
        Value::Text(day) => day.clone(),
        _ => String::new(),
    };
    day(&row[5]) <= day(to) && day(&row[6]) >= day(from)
}

fn reaches(row: &Row, person: &Value) -> bool {
    !job_row(&row[3], person).is_empty()
}

impl Store {
    fn tag(&self, equipment: &Value) -> Value {
        self.things
            .iter()
            .find(|thing| thing[1] == *equipment)
            .map_or(Value::Null, |thing| thing[2].clone())
    }

    fn thing_rows(&mut self, sql: &'static str, p: &[Value]) -> Vec<Row> {
        match sql {
            equipment::LIST => {
                let mut rows: Vec<Row> = self.things.iter().filter(|row| row[0] == p[0]).cloned().collect();
                rows.sort_by_key(|row| format!("{:?}", row[2]));
                rows.into_iter().map(|row| row[1..].to_vec()).collect()
            }
            equipment::LOCK => keyed(&self.things, p),
            equipment::ADD => {
                self.things.push(p.to_vec());
                Vec::new()
            }
            equipment::SAVE => self
                .things
                .iter_mut()
                .filter(|row| row[..2] == p[..2])
                .map(|row| {
                    row.clone_from_slice(p);
                    vec![p[1].clone()]
                })
                .collect(),
            _ => self.booking_rows(sql, p),
        }
    }

    fn booking_rows(&mut self, sql: &'static str, p: &[Value]) -> Vec<Row> {
        let live = |row: &&Row| row[0] == p[0] && row[7] == text("booked");
        match sql {
            equipment::CLASHES => self
                .bookings
                .iter()
                .filter(live)
                .filter(|row| row[2] == p[1] && meets(row, &p[2], &p[3]))
                .map(|row| vec![row[1].clone()])
                .collect(),
            equipment::BOOK => {
                self.bookings.push([p, &[text("booked")]].concat());
                Vec::new()
            }
            equipment::BOOKING => keyed(&self.bookings, p),
            equipment::CANCEL => self
                .bookings
                .iter_mut()
                .filter(|row| row[..2] == p[..2] && row[7] == text("booked"))
                .map(|row| {
                    row[7] = text("cancelled");
                    vec![p[1].clone()]
                })
                .collect(),
            equipment::TIMELINE => {
                let mut rows: Vec<&Row> = self
                    .bookings
                    .iter()
                    .filter(live)
                    .filter(|row| meets(row, &p[1], &p[2]) && reaches(row, &p[3]))
                    .collect();
                rows.sort_by_key(|row| format!("{:?} {:?}", self.tag(&row[2]), row[5]));
                rows.into_iter().map(|row| row[1..7].to_vec()).collect()
            }
            _ => self.need_rows(sql, p),
        }
    }

    fn need_rows(&mut self, sql: &'static str, p: &[Value]) -> Vec<Row> {
        match sql {
            equipment::NEED => {
                self.needs.push([p, &[Value::Null]].concat());
                Vec::new()
            }
            equipment::GET_NEED => keyed(&self.needs, p),
            equipment::NEEDS => self
                .needs
                .iter()
                .filter(|row| row[0] == p[0] && row[7] == Value::Null)
                .filter(|row| !job_row(&row[2], &p[1]).is_empty())
                .map(|row| row[1..7].to_vec())
                .collect(),
            equipment::FILL => self
                .needs
                .iter_mut()
                .filter(|row| row[..2] == p[..2] && row[7] == Value::Null)
                .map(|row| {
                    row[7] = p[2].clone();
                    vec![p[1].clone()]
                })
                .collect(),
            equipment::RELEASE => {
                for row in self.needs.iter_mut().filter(|row| row[7] == p[1]) {
                    row[7] = Value::Null;
                }
                Vec::new()
            }
            _ => Vec::new(),
        }
    }
}

impl Tx for Store {
    fn run(&mut self, sql: &'static str, p: &[Value]) -> Result<Vec<Row>, Fail> {
        Ok(match self.trail.run(sql, p) {
            Some(rows) => rows,
            None if sql == tasks::GET => task_row(&p[1]),
            None => self.thing_rows(sql, p),
        })
    }
}

type TestHost = memory::TestHost<Store>;

fn answer(openworks: &Openworks<&TestHost>, path: &str, credential: &str, body: &str) -> (u16, String) {
    let response = send(openworks, path, credential, body);
    (response.status, response.body)
}

fn thing(openworks: &Openworks<&TestHost>, tag: &str, kind: &str) -> String {
    ok(send(
        openworks,
        "/equipment/save",
        "owner",
        &format!("\n{tag}\n{tag}\n{kind}\nYard\n\n"),
    ))
}

fn book(openworks: &Openworks<&TestHost>, body: &str) -> (u16, String) {
    answer(openworks, "/equipment/book", "manager", body)
}

#[test]
fn a_thing_keeps_its_tag_name_kind_place_holder_job_and_status() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();

    let van = ok(send(
        &openworks,
        "/equipment/save",
        "owner",
        "\nV-1\nVan one\nVan\nYard\n\n",
    ));
    let lift = ok(send(
        &openworks,
        "/equipment/save",
        "manager",
        "\nL-2\nLift two\nLift\nBay 4\nj1\nout_of_service",
    ));
    let moved = ok(send(
        &openworks,
        "/equipment/save",
        "member",
        &format!("{van}\nV-1\nVan one\nVan\nBay 4\nj1\nin_service"),
    ));

    assert_eq!(moved, van);
    assert_eq!(
        ok(send(&openworks, "/equipment/list", "viewer", "")),
        format!(
            r#"[{{"id":"{lift}","tag":"L-2","name":"Lift two","kind":"Lift","place":"Bay 4","holder_job_id":"j1","status":"out_of_service","ext":{{}}}},{{"id":"{van}","tag":"V-1","name":"Van one","kind":"Van","place":"Bay 4","holder_job_id":"j1","status":"in_service","ext":{{}}}}]"#
        )
    );
    assert_eq!(
        answer(
            &openworks,
            "/equipment/save",
            "owner",
            "\nV-2\nVan two\nVan\n\n\nbroken"
        ),
        (400, "invalid: status is in_service or out_of_service".to_owned())
    );
    assert_eq!(
        answer(&openworks, "/equipment/save", "owner", "\n\nVan two"),
        (400, "invalid: a tag is needed".to_owned())
    );
    assert_eq!(
        answer(&openworks, "/equipment/save", "member", "\nV-2\nVan two\n\n\nj2\n").0,
        404
    );
    assert_eq!(answer(&openworks, "/equipment/save", "viewer", "\nV-2\nVan two").0, 403);
    assert_eq!(host.store().trail.audits, 3);
    assert_eq!(host.store().trail.events, vec![text("equipment.save"); 3]);
}

#[test]
fn two_live_bookings_of_one_thing_cannot_overlap() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    let van = thing(&openworks, "V-1", "Van");
    let other = thing(&openworks, "V-2", "Van");

    let first = ok(send(
        &openworks,
        "/equipment/book",
        "member",
        &format!("{van}\nj1\nt1\n2026-01-05\n2026-01-09"),
    ));

    assert_eq!(
        book(&openworks, &format!("{van}\nj2\n\n2026-01-09\n2026-01-12")),
        (400, format!("invalid: V-1 is booked on those days by {first}"))
    );
    assert_eq!(book(&openworks, &format!("{van}\nj2\n\n2026-01-01\n2026-01-20")).0, 400);
    let next = book(&openworks, &format!("{van}\nj2\n\n2026-01-10\n2026-01-12"));
    assert_eq!(next.0, 200, "{}", next.1);
    assert_eq!(
        book(&openworks, &format!("{other}\nj2\n\n2026-01-05\n2026-01-09")).0,
        200
    );
    assert_eq!(
        answer(&openworks, "/equipment/cancel", "manager", &first),
        (200, first.clone())
    );
    assert_eq!(
        answer(&openworks, "/equipment/cancel", "manager", &first),
        (400, format!("invalid: booking {first} is cancelled"))
    );
    assert_eq!(book(&openworks, &format!("{van}\nj2\n\n2026-01-05\n2026-01-09")).0, 200);
    let live = host
        .store()
        .bookings
        .iter()
        .filter(|row| row[7] == text("booked"))
        .count();
    assert_eq!(live, 3);
}

#[test]
fn a_booking_is_refused_off_its_job_out_of_order_or_out_of_service() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    let van = thing(&openworks, "V-1", "Van");
    let other = thing(&openworks, "V-2", "Van");

    assert_eq!(
        book(&openworks, &format!("{van}\nj2\nt1\n2026-01-20\n2026-01-21")),
        (400, "invalid: task t1 is not on job j2".to_owned())
    );
    assert_eq!(
        book(&openworks, &format!("{van}\nj2\n\n2026-01-22\n2026-01-21")),
        (400, "invalid: 2026-01-21 is before 2026-01-22".to_owned())
    );
    assert_eq!(
        answer(
            &openworks,
            "/equipment/book",
            "member",
            &format!("{van}\nj2\n\n2026-01-20\n2026-01-21")
        )
        .0,
        404
    );
    ok(send(
        &openworks,
        "/equipment/save",
        "owner",
        &format!("{other}\nV-2\nV-2\nVan\nYard\n\nout_of_service"),
    ));
    assert_eq!(
        book(&openworks, &format!("{other}\nj2\n\n2026-02-01\n2026-02-02")),
        (400, "invalid: V-2 is out of service".to_owned())
    );
    assert!(host.store().bookings.is_empty());
}

#[test]
fn a_need_stays_open_until_a_booking_of_its_kind_fills_it() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    let van = thing(&openworks, "V-1", "Van");
    let lift = thing(&openworks, "L-1", "Lift");

    let need = ok(send(
        &openworks,
        "/equipment/need",
        "member",
        "j1\nt1\nVan\n2026-01-05\n2026-01-09",
    ));

    assert_eq!(
        ok(send(&openworks, "/equipment/needs", "viewer", "")),
        format!(
            r#"[{{"id":"{need}","job_id":"j1","task_id":"t1","kind":"Van","starts_on":"2026-01-05","ends_on":"2026-01-09","ext":{{}}}}]"#
        )
    );
    assert_eq!(
        answer(&openworks, "/equipment/fill", "manager", &format!("{need}\n{lift}")),
        (400, "invalid: L-1 is not of the kind the need names".to_owned())
    );
    let booking = ok(send(
        &openworks,
        "/equipment/fill",
        "manager",
        &format!("{need}\n{van}"),
    ));
    assert_eq!(ok(send(&openworks, "/equipment/needs", "viewer", "")), "[]");
    assert_eq!(
        ok(send(
            &openworks,
            "/equipment/timeline",
            "viewer",
            "2026-01-01\n2026-01-31"
        )),
        format!(
            r#"[{{"id":"{booking}","equipment_id":"{van}","job_id":"j1","task_id":"t1","starts_on":"2026-01-05","ends_on":"2026-01-09","ext":{{}}}}]"#
        )
    );
    assert_eq!(
        answer(&openworks, "/equipment/fill", "manager", &format!("{need}\n{van}")),
        (400, format!("invalid: need {need} is booked"))
    );
    ok(send(&openworks, "/equipment/cancel", "manager", &booking));
    assert!(ok(send(&openworks, "/equipment/needs", "viewer", "")).contains(&need));
    assert_eq!(
        answer(&openworks, "/equipment/need", "owner", "j1\n\n\n2026-01-05\n2026-01-09"),
        (400, "invalid: a need names a kind".to_owned())
    );
}

#[test]
fn the_timeline_lists_the_live_bookings_meeting_the_days_thing_by_thing() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    let van = thing(&openworks, "V-1", "Van");
    let lift = thing(&openworks, "L-1", "Lift");
    let late = ok(send(
        &openworks,
        "/equipment/book",
        "owner",
        &format!("{van}\nj1\n\n2026-01-12\n2026-01-14"),
    ));
    let early = ok(send(
        &openworks,
        "/equipment/book",
        "owner",
        &format!("{van}\nj2\n\n2026-01-02\n2026-01-06"),
    ));
    let lifted = ok(send(
        &openworks,
        "/equipment/book",
        "owner",
        &format!("{lift}\nj1\n\n2026-01-08\n2026-01-08"),
    ));
    let outside = ok(send(
        &openworks,
        "/equipment/book",
        "owner",
        &format!("{lift}\nj1\n\n2026-02-01\n2026-02-03"),
    ));
    let cancelled = ok(send(
        &openworks,
        "/equipment/book",
        "owner",
        &format!("{van}\nj1\n\n2026-01-08\n2026-01-09"),
    ));
    ok(send(&openworks, "/equipment/cancel", "owner", &cancelled));

    let ids = |credential: &str| {
        let timeline = ok(send(
            &openworks,
            "/equipment/timeline",
            credential,
            "2026-01-05\n2026-01-12",
        ));
        [&lifted, &early, &late, &outside, &cancelled]
            .into_iter()
            .filter_map(|id| timeline.find(&format!(r#""id":"{id}""#)).map(|at| (at, id.clone())))
            .collect::<std::collections::BTreeMap<_, _>>()
            .into_values()
            .collect::<Vec<_>>()
    };

    assert_eq!(ids("viewer"), [lifted.clone(), early.clone(), late.clone()]);
    assert_eq!(ids("member"), [lifted, late]);
    assert_eq!(
        answer(&openworks, "/equipment/timeline", "viewer", "2026-01-12\n2026-01-05"),
        (400, "invalid: 2026-01-05 is before 2026-01-12".to_owned())
    );
}
