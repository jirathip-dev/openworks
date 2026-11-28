mod memory;
mod table;

use memory::{ok, send};
use openworks::Openworks;
use openworks_core::{Fail, Row, Tx, Value};
use openworks_db::{equipment, jobs, tasks};
use table::{Trail, change, select, text};

#[derive(Clone, Default)]
struct Store {
    things: Vec<Row>,
    bookings: Vec<Row>,
    needs: Vec<Row>,
    trail: Trail,
}

fn task_row(id: &Value) -> Vec<Row> {
    let mut row = vec![id.clone(), text("j1")];
    row.resize(12, Value::Null);
    if *id == text("t1") { vec![row] } else { Vec::new() }
}

fn reaches(job: &Value, person: &Value) -> bool {
    [text("j1"), text("j2")].contains(job) && (*person == Value::Null || *job == text("j1"))
}

fn meets(row: &Row, from: &Value, to: &Value) -> bool {
    let day = |value: &Value| format!("{value:?}");
    day(&row[5]) <= day(to) && day(&row[6]) >= day(from)
}

fn keyed(p: &[Value]) -> impl Fn(&Row) -> bool + '_ {
    move |row: &Row| row[..2] == p[..2]
}

impl Store {
    fn thing_rows(&mut self, sql: &'static str, p: &[Value]) -> Vec<Row> {
        match sql {
            jobs::GET if reaches(&p[1], &p[2]) => {
                let mut row = vec![p[1].clone(), text("p1")];
                row.resize(13, Value::Null);
                vec![row]
            }
            equipment::LIST => {
                self.things.sort_by_key(|row| format!("{:?}", row[2]));
                select(&self.things, |row| row[0] == p[0])
            }
            equipment::LOCK => select(&self.things, keyed(p)),
            equipment::ADD => {
                self.things.push(p.to_vec());
                Vec::new()
            }
            equipment::SAVE => change(&mut self.things, keyed(p), |row| row.clone_from_slice(p)),
            _ => self.booking_rows(sql, p),
        }
    }

    fn booking_rows(&mut self, sql: &'static str, p: &[Value]) -> Vec<Row> {
        let live = |row: &Row| row[0] == p[0] && row[7] == text("booked");
        match sql {
            equipment::CLASHES => select(&self.bookings, |row| {
                live(row) && row[2] == p[1] && meets(row, &p[2], &p[3])
            }),
            equipment::BOOK => {
                self.bookings.push([p, &[text("booked")]].concat());
                Vec::new()
            }
            equipment::BOOKING => select(&self.bookings, keyed(p)),
            equipment::CANCEL => change(
                &mut self.bookings,
                |row| keyed(p)(row) && live(row),
                |row| row[7] = text("cancelled"),
            ),
            equipment::TIMELINE => {
                let tag = |row: &Row| {
                    self.things
                        .iter()
                        .find(|thing| thing[1] == row[2])
                        .map(|thing| thing[2].clone())
                };
                let mut rows: Vec<&Row> = self.bookings.iter().filter(|row| live(row)).collect();
                rows.retain(|row| meets(row, &p[1], &p[2]) && reaches(&row[3], &p[3]));
                rows.sort_by_key(|row| format!("{:?} {:?}", tag(row), row[5]));
                rows.into_iter().map(|row| row[1..7].to_vec()).collect()
            }
            _ => self.need_rows(sql, p),
        }
    }

    fn need_rows(&mut self, sql: &'static str, p: &[Value]) -> Vec<Row> {
        let open = |row: &Row| row[7] == Value::Null;
        match sql {
            equipment::NEED => {
                self.needs.push([p, &[Value::Null]].concat());
                Vec::new()
            }
            equipment::GET_NEED => select(&self.needs, keyed(p)),
            equipment::NEEDS => {
                let rows = select(&self.needs, |row| {
                    row[0] == p[0] && open(row) && reaches(&row[2], &p[1])
                });
                rows.into_iter().map(|row| row[..6].to_vec()).collect()
            }
            equipment::FILL => change(
                &mut self.needs,
                |row| keyed(p)(row) && open(row),
                |row| row[7] = p[2].clone(),
            ),
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

fn at(openworks: &Openworks<&TestHost>, operation: &str, who: &str, body: &str) -> (u16, String) {
    let response = send(openworks, &format!("/equipment/{operation}"), who, body);
    (response.status, response.body)
}

fn done(openworks: &Openworks<&TestHost>, operation: &str, who: &str, body: &str) -> String {
    ok(send(openworks, &format!("/equipment/{operation}"), who, body))
}

fn thing(openworks: &Openworks<&TestHost>, tag: &str, kind: &str) -> String {
    done(openworks, "save", "owner", &format!("\n{tag}\n{tag}\n{kind}\nYard\n\n"))
}

fn ok_body((status, body): (u16, String)) -> String {
    assert_eq!(status, 200, "{body}");
    body
}

fn ids(listed: &str) -> Vec<&str> {
    let rest = listed.split(r#"{"id":""#).skip(1);
    rest.filter_map(|rest| rest.split('"').next()).collect()
}

#[test]
fn a_thing_keeps_its_tag_name_kind_place_holder_job_and_status() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();

    let save = |who: &str, body: &str| at(&openworks, "save", who, body);
    let van = ok_body(save("owner", "\nV-1\nVan one\nVan\nYard\n\n"));
    let lift = ok_body(save("manager", "\nL-2\nLift two\nLift\nBay 4\nj1\nout_of_service"));
    let moved = ok_body(save(
        "member",
        &format!("{van}\nV-1\nVan one\nVan\nBay 4\nj1\nin_service"),
    ));

    assert_eq!(moved, van);
    assert_eq!(
        done(&openworks, "list", "viewer", ""),
        format!(
            r#"[{{"id":"{lift}","tag":"L-2","name":"Lift two","kind":"Lift","place":"Bay 4","holder_job_id":"j1","status":"out_of_service","ext":{{}}}},{{"id":"{van}","tag":"V-1","name":"Van one","kind":"Van","place":"Bay 4","holder_job_id":"j1","status":"in_service","ext":{{}}}}]"#
        )
    );
    let status = (400, "invalid: status is in_service or out_of_service".to_owned());
    assert_eq!(save("owner", "\nV-2\nVan two\nVan\n\n\nbroken"), status);
    assert_eq!(
        save("owner", "\n\nVan two"),
        (400, "invalid: a tag is needed".to_owned())
    );
    assert_eq!(save("member", "\nV-2\nVan two\n\n\nj2\n").0, 404);
    assert_eq!(save("viewer", "\nV-2\nVan two").0, 403);
    assert_eq!(host.store().trail.audits, 3);
    let saves = host
        .store()
        .trail
        .events
        .iter()
        .filter(|(kind, _)| *kind == text("equipment.save"))
        .count();
    assert_eq!(saves, 3);
}

#[test]
fn two_live_bookings_of_one_thing_cannot_overlap() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    let van = thing(&openworks, "V-1", "Van");
    let other = thing(&openworks, "V-2", "Van");
    let book = |thing: &str, days: &str| at(&openworks, "book", "manager", &format!("{thing}\nj2\n\n{days}"));

    let first = ok_body(at(
        &openworks,
        "book",
        "member",
        &format!("{van}\nj1\nt1\n2026-01-05\n2026-01-09"),
    ));

    let clash = (400, format!("invalid: V-1 is booked on those days by {first}"));
    assert_eq!(book(&van, "2026-01-09\n2026-01-12"), clash);
    assert_eq!(book(&van, "2026-01-01\n2026-01-20"), clash);
    assert_eq!(book(&van, "2026-01-10\n2026-01-12").0, 200);
    assert_eq!(book(&other, "2026-01-05\n2026-01-09").0, 200);
    assert_eq!(at(&openworks, "cancel", "manager", &first), (200, first.clone()));
    let again = (400, format!("invalid: booking {first} is cancelled"));
    assert_eq!(at(&openworks, "cancel", "manager", &first), again);
    assert_eq!(book(&van, "2026-01-05\n2026-01-09").0, 200);
    assert_eq!(host.store().trail.events.len(), 7);
}

#[test]
fn a_booking_is_refused_off_its_job_out_of_order_or_out_of_service() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    let van = thing(&openworks, "V-1", "Van");
    let book = |who: &str, body: &str| at(&openworks, "book", who, &format!("{van}\n{body}"));

    let off_job = (400, "invalid: task t1 is not on job j2".to_owned());
    assert_eq!(book("manager", "j2\nt1\n2026-01-20\n2026-01-21"), off_job);
    let order = (400, "invalid: 2026-01-21 is before 2026-01-22".to_owned());
    assert_eq!(book("manager", "j2\n\n2026-01-22\n2026-01-21"), order);
    assert_eq!(book("member", "j2\n\n2026-01-20\n2026-01-21").0, 404);
    let broken = format!("{van}\nV-1\nV-1\nVan\nYard\n\nout_of_service");
    done(&openworks, "save", "owner", &broken);
    let out = (400, "invalid: V-1 is out of service".to_owned());
    assert_eq!(book("manager", "j2\n\n2026-02-01\n2026-02-02"), out);
    assert_eq!(host.store().bookings.len(), 0);
}

#[test]
fn a_need_stays_open_until_a_booking_of_its_kind_fills_it() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    let van = thing(&openworks, "V-1", "Van");
    let lift = thing(&openworks, "L-1", "Lift");

    let need = done(&openworks, "need", "member", "j1\nt1\nVan\n2026-01-05\n2026-01-09");

    assert_eq!(
        done(&openworks, "needs", "viewer", ""),
        format!(
            r#"[{{"id":"{need}","job_id":"j1","task_id":"t1","kind":"Van","starts_on":"2026-01-05","ends_on":"2026-01-09","ext":{{}}}}]"#
        )
    );
    let kind = (400, "invalid: L-1 is not of the kind the need names".to_owned());
    assert_eq!(at(&openworks, "fill", "manager", &format!("{need}\n{lift}")), kind);
    let booking = done(&openworks, "fill", "manager", &format!("{need}\n{van}"));
    assert_eq!(done(&openworks, "needs", "viewer", ""), "[]");
    assert_eq!(
        done(&openworks, "timeline", "viewer", "2026-01-01\n2026-01-31"),
        format!(
            r#"[{{"id":"{booking}","equipment_id":"{van}","job_id":"j1","task_id":"t1","starts_on":"2026-01-05","ends_on":"2026-01-09","ext":{{}}}}]"#
        )
    );
    let filled = (400, format!("invalid: need {need} is booked"));
    assert_eq!(at(&openworks, "fill", "manager", &format!("{need}\n{van}")), filled);
    done(&openworks, "cancel", "manager", &booking);
    assert_eq!(ids(&done(&openworks, "needs", "viewer", "")), [need.as_str()]);
    let unnamed = (400, "invalid: a need names a kind".to_owned());
    let nameless = at(&openworks, "need", "owner", "j1\n\n\n2026-01-05\n2026-01-09");
    assert_eq!(nameless, unnamed);
}

#[test]
fn the_timeline_lists_the_live_bookings_meeting_the_days_thing_by_thing() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    let van = thing(&openworks, "V-1", "Van");
    let lift = thing(&openworks, "L-1", "Lift");
    let book = |thing: &str, body: &str| done(&openworks, "book", "owner", &format!("{thing}\n{body}"));
    let late = book(&van, "j1\n\n2026-01-12\n2026-01-14");
    let early = book(&van, "j2\n\n2026-01-02\n2026-01-06");
    let lifted = book(&lift, "j1\n\n2026-01-08\n2026-01-08");
    book(&lift, "j1\n\n2026-02-01\n2026-02-03");
    let cancelled = book(&van, "j1\n\n2026-01-08\n2026-01-09");
    done(&openworks, "cancel", "owner", &cancelled);

    let timeline = |who: &str| done(&openworks, "timeline", who, "2026-01-05\n2026-01-12");

    assert_eq!(ids(&timeline("viewer")), [&lifted, &early, &late]);
    assert_eq!(ids(&timeline("member")), [&lifted, &late]);
    let order = (400, "invalid: 2026-01-05 is before 2026-01-12".to_owned());
    assert_eq!(at(&openworks, "timeline", "viewer", "2026-01-12\n2026-01-05"), order);
}
