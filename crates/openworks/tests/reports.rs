mod memory;
mod table;

use std::collections::BTreeMap;

use memory::{ok, send};
use openworks::{Openworks, Response};
use openworks_core::{Fail, Row, Tx, Value};
use openworks_db::{jobs, reports, series, tasks, workers};
use table::{Trail, change, select, text};

#[derive(Clone, Default)]
struct Store {
    kinds: Vec<Row>,
    reports: Vec<Row>,
    hours: Vec<Row>,
    progress: Vec<Row>,
    archived: Vec<Value>,
    last: i64,
    trail: Trail,
}

fn int(value: &Value) -> i64 {
    match value {
        Value::Int(int) => *int,
        _ => 0,
    }
}

fn plain(value: &Value) -> String {
    match value {
        Value::Text(text) => text.clone(),
        _ => String::new(),
    }
}

fn of(table: &[Row], p: &[Value]) -> Vec<Row> {
    select(table, |row| row[..2] == p[..2])
        .into_iter()
        .map(|row| row[1..].to_vec())
        .collect()
}

impl Store {
    fn by_day(&self, p: &[Value]) -> Vec<Row> {
        let mut sums: BTreeMap<(String, String), Row> = BTreeMap::new();
        let counted = self
            .reports
            .iter()
            .filter(|r| r[0] == p[0] && r[2] == p[1] && r[5] != text("voided"));
        for report in counted {
            for hour in self.hours.iter().filter(|h| h[..2] == report[..2]) {
                let key = (plain(&report[3]), plain(&hour[4]));
                let sum = sums
                    .entry(key)
                    .or_insert_with(|| vec![report[1].clone(), report[3].clone(), hour[4].clone(), Value::Int(0)]);
                sum[3] = Value::Int(int(&sum[3]) + int(&hour[5]));
            }
        }
        sums.into_values().collect()
    }

    fn report_rows(&mut self, sql: &'static str, p: &[Value]) -> Vec<Row> {
        match sql {
            reports::LIST => {
                let mut rows = select(&self.reports, |r| r[0] == p[0] && r[2] == p[1]);
                rows.sort_by_key(|r| plain(&r[2]));
                rows
            }
            reports::GET => select(&self.reports, |r| r[..2] == p[..2]),
            reports::FIND => select(&self.reports, |r| r[0] == p[0] && r[2] == p[1] && r[3] == p[2]),
            reports::ADD => {
                self.reports.push([p, &[text("draft"), Value::Int(1)]].concat());
                Vec::new()
            }
            reports::EDIT => change(
                &mut self.reports,
                |r| r[..2] == p[..2],
                |r| r[6] = Value::Int(int(&r[6]) + 1),
            ),
            reports::HOURS => {
                let mut rows = of(&self.hours, p);
                rows.sort_by_key(|h| (plain(&h[1]), plain(&h[2]), plain(&h[3])));
                rows
            }
            reports::CLEAR_HOURS => {
                self.hours.retain(|h| h[..2] != p[..2]);
                Vec::new()
            }
            reports::ADD_HOURS => {
                self.hours.push(p.to_vec());
                Vec::new()
            }
            reports::PROGRESS => of(&self.progress, p),
            reports::CLEAR_PROGRESS => {
                self.progress.retain(|g| g[..2] != p[..2]);
                Vec::new()
            }
            reports::ADD_PROGRESS => {
                self.progress.push(p.to_vec());
                Vec::new()
            }
            reports::BY_DAY => self.by_day(p),
            reports::REPORTED => {
                let on_task = self.hours.iter().filter(|h| h[0] == p[0] && h[3] == p[1]);
                vec![vec![Value::Int(on_task.map(|h| int(&h[5])).sum())]]
            }
            _ => Vec::new(),
        }
    }
}

impl Tx for Store {
    fn run(&mut self, sql: &'static str, p: &[Value]) -> Result<Vec<Row>, Fail> {
        if let Some(rows) = self.trail.run(sql, p) {
            return Ok(rows);
        }
        let known = |ids: &[&str]| p[0] == text("o1") && ids.iter().any(|id| p[1] == text(id));
        let job = |status: &str| vec![vec![p[1].clone(), text("p1"), text("J-1"), text(status)]];
        let task = |job: &str| vec![vec![p[1].clone(), text(job)]];
        Ok(match sql {
            jobs::GET if known(&["j1", "j2"]) => job("open"),
            jobs::GET if known(&["jp"]) => job("proposed"),
            workers::GET if known(&["ada", "bo"]) => vec![vec![p[1].clone()]],
            tasks::GET if known(&["t1", "t2"]) => task("j1"),
            tasks::GET if known(&["t9"]) => task("j2"),
            tasks::ARCHIVE => {
                self.archived.push(p[1].clone());
                vec![vec![p[1].clone()]]
            }
            series::NEXT => {
                self.last += 1;
                vec![vec![Value::Int(self.last)]]
            }
            reports::KINDS => {
                let mut rows = select(&self.kinds, |kind| kind[0] == p[0]);
                rows.sort_by_key(|kind| (int(&kind[2]), plain(&kind[0])));
                rows
            }
            reports::KIND => of(&self.kinds, p).into_iter().map(|_| vec![p[1].clone()]).collect(),
            reports::KIND_SAVE => {
                self.kinds.retain(|kind| kind[..2] != p[..2]);
                self.kinds.push(p.to_vec());
                Vec::new()
            }
            _ => self.report_rows(sql, p),
        })
    }
}

type TestHost = memory::TestHost<Store>;

fn report(id: &str, day: &str, number: u8, version: u8) -> String {
    format!(
        r#"{{"id":"{id}","job_id":"j1","day":"{day}","number":"R-2023-{number}","status":"draft","version":{version},"ext":{{}}}}"#
    )
}

fn refused(post: impl Fn(&str, &str, &str) -> Response, cases: &[(&str, &str, &str, u16)]) {
    for (path, who, body, status) in cases {
        assert_eq!(post(path, who, body).status, *status, "{path} as {who}: {body}");
    }
}

fn kinds(post: impl Fn(&str, &str, &str) -> Response) {
    ok(post("/projects/hour-kind-save", "owner", "regular\nRegular\n1"));
    ok(post("/projects/hour-kind-save", "owner", "overtime\nOvertime\n2"));
}

fn reported(post: impl Fn(&str, &str, &str) -> Response) -> String {
    kinds(&post);
    let id = ok(post("/daily-reports/save", "owner", "j1\n2026-01-05"));
    ok(post(
        "/daily-reports/set-hours",
        "owner",
        &format!("{id}\nada\tt1\tregular\t800"),
    ));
    id
}

fn status(host: &TestHost, id: &str, status: &str) {
    let mut store = host.store();
    let row = store.reports.iter_mut().find(|r| r[1] == text(id)).unwrap();
    row[5] = text(status);
}

#[test]
fn the_company_keeps_its_own_kinds_of_hour() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    let post = |path: &str, who: &str, body: &str| send(&openworks, path, who, body);

    kinds(post);
    assert_eq!(
        ok(post("/projects/hour-kind-save", "owner", "regular\nNormal\n3")),
        "regular"
    );
    assert_eq!(
        ok(post("/projects/hour-kinds", "viewer", "")),
        concat!(
            r#"[{"id":"overtime","name":"Overtime","sort":2,"ext":{}},"#,
            r#"{"id":"regular","name":"Normal","sort":3,"ext":{}}]"#
        )
    );
    refused(
        post,
        &[
            ("/projects/hour-kind-save", "owner", "night shift\nNight\n", 400),
            ("/projects/hour-kind-save", "owner", "night\n\n", 400),
            ("/projects/hour-kind-save", "owner", "night\nNight\n-1", 400),
            ("/projects/hour-kind-save", "manager", "night\nNight\n", 403),
        ],
    );
    assert_eq!(host.store().kinds.len(), 2);
    assert_eq!(host.store().trail.audits, 3);
    assert_eq!(host.store().trail.events.len(), 3);
}

#[test]
fn a_job_has_one_numbered_report_a_day() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    let post = |path: &str, who: &str, body: &str| send(&openworks, path, who, body);

    let tuesday = ok(post("/daily-reports/save", "member", "j1\n2026-01-06"));
    let monday = ok(post("/daily-reports/save", "owner", "j1\n2026-01-05"));
    assert_eq!(ok(post("/daily-reports/save", "owner", "j1\n2026-01-06")), tuesday);
    let listed = format!(
        "[{},{}]",
        report(&monday, "2026-01-05", 2, 1),
        report(&tuesday, "2026-01-06", 1, 1)
    );
    assert_eq!(ok(post("/daily-reports/list", "viewer", "j1")), listed);

    refused(
        post,
        &[
            ("/daily-reports/save", "owner", "jp\n2026-01-05", 400),
            ("/daily-reports/save", "owner", "j1\n5 Jan", 400),
            ("/daily-reports/save", "owner", "nojob\n2026-01-05", 404),
            ("/daily-reports/save", "other", "j1\n2026-01-05", 404),
            ("/daily-reports/save", "viewer", "j1\n2026-01-07", 403),
            ("/daily-reports/list", "other", "j1", 404),
        ],
    );
    assert_eq!(host.store().reports.len(), 2);
    assert_eq!(host.store().last, 2);
}

#[test]
fn a_report_holds_hours_per_worker_task_and_kind_and_each_edit_is_a_new_version() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    let post = |path: &str, who: &str, body: &str| send(&openworks, path, who, body);
    kinds(post);
    let id = ok(post("/daily-reports/save", "owner", "j1\n2026-01-05"));

    let hours = format!("{id}\nada\tt1\tregular\t800\nada\tt1\tovertime\t150\nbo\tt2\tregular\t400");
    assert_eq!(ok(post("/daily-reports/set-hours", "member", &hours)), id);
    assert_eq!(
        ok(post("/daily-reports/hours", "viewer", &id)),
        concat!(
            r#"[{"id":"ada","task_id":"t1","kind":"overtime","hours":150,"ext":{}},"#,
            r#"{"id":"ada","task_id":"t1","kind":"regular","hours":800,"ext":{}},"#,
            r#"{"id":"bo","task_id":"t2","kind":"regular","hours":400,"ext":{}}]"#
        )
    );
    assert_eq!(ok(post("/daily-reports/set-hours", "owner", &hours)), id);
    assert_eq!(host.store().hours.len(), 3);
    let listed = ok(post("/daily-reports/list", "viewer", "j1"));
    assert_eq!(listed, format!("[{}]", report(&id, "2026-01-05", 1, 3)));

    assert_eq!(host.store().trail.events.len(), 5);
}

#[test]
fn hours_name_a_known_worker_a_task_of_the_job_a_kind_and_more_than_none() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    let post = |path: &str, who: &str, body: &str| send(&openworks, path, who, body);
    let id = reported(post);

    for (who, lines, status) in [
        ("owner", "nobody\tt1\tregular\t800", 404),
        ("owner", "ada\tt9\tregular\t800", 400),
        ("owner", "ada\tt1\tnight\t800", 404),
        ("owner", "ada\tt1\tregular\t0", 400),
        ("owner", "ada\tt1\tregular\t7.5", 400),
        ("owner", "ada\tt1\tregular", 400),
        ("owner", "ada\tt1\tregular\t1\nada\tt1\tregular\t2", 400),
        ("viewer", "ada\tt1\tregular\t800", 403),
        ("other", "ada\tt1\tregular\t800", 404),
    ] {
        let answered = post("/daily-reports/set-hours", who, &format!("{id}\n{lines}"));
        assert_eq!(answered.status, status, "{who}: {lines}: {}", answered.body);
    }
    assert_eq!(
        post("/daily-reports/set-hours", "owner", "nothing\nada\tt1\tregular\t1").status,
        404
    );
    assert_eq!(post("/daily-reports/hours", "other", &id).status, 404);

    status(&host, &id, "approved");
    let approved = post(
        "/daily-reports/set-hours",
        "owner",
        &format!("{id}\nada\tt1\tregular\t1"),
    );
    assert_eq!(
        approved.body,
        format!("invalid: report {id} is approved and does not change")
    );
    assert_eq!(host.store().hours.len(), 1);
    assert_eq!(host.store().hours[0][5], Value::Int(800));
    assert_eq!(host.store().reports[0][6], Value::Int(2));
}

#[test]
fn a_report_declares_progress_per_task() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    let post = |path: &str, who: &str, body: &str| send(&openworks, path, who, body);
    let id = ok(post("/daily-reports/save", "owner", "j1\n2026-01-05"));

    let set = |lines: &str| post("/daily-reports/set-progress", "member", &format!("{id}\n{lines}"));
    assert_eq!(ok(set("t1\t40\nt2\t100")), id);
    assert_eq!(ok(set("t1\t55")), id);
    assert_eq!(
        ok(post("/daily-reports/progress", "viewer", &id)),
        r#"[{"id":"t1","percent":55,"ext":{}}]"#
    );

    let line = |line: &str| format!("{id}\n{line}");
    refused(
        post,
        &[
            ("/daily-reports/set-progress", "owner", &line("t1\t101"), 400),
            ("/daily-reports/set-progress", "owner", &line("t1\t-5"), 400),
            ("/daily-reports/set-progress", "owner", &line("t9\t10"), 400),
            ("/daily-reports/set-progress", "owner", &line("t7\t10"), 404),
            ("/daily-reports/set-progress", "owner", &line("t1\t10\nt1\t20"), 400),
            ("/daily-reports/set-progress", "viewer", &line("t1\t10"), 403),
        ],
    );
    status(&host, &id, "voided");
    assert_eq!(
        post("/daily-reports/set-progress", "owner", &line("t1\t60")).status,
        400
    );
    assert_eq!(host.store().progress.len(), 1);
    assert_eq!(host.store().reports[0][6], Value::Int(3));
}

#[test]
fn a_job_s_hours_add_up_by_day_and_kind_without_voided_reports() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    let post = |path: &str, who: &str, body: &str| send(&openworks, path, who, body);
    kinds(post);
    let mut ids = Vec::new();
    for (day, hours) in [
        (
            "2026-01-06",
            "ada\tt1\tregular\t800\nbo\tt2\tregular\t400\nbo\tt2\tovertime\t200",
        ),
        ("2026-01-05", "ada\tt1\tregular\t700"),
        ("2026-01-07", "ada\tt1\tregular\t500"),
    ] {
        let id = ok(post("/daily-reports/save", "owner", &format!("j1\n{day}")));
        ok(post("/daily-reports/set-hours", "owner", &format!("{id}\n{hours}")));
        ids.push(id);
    }
    status(&host, &ids[2], "voided");

    let row = |id: &str, day: &str, kind: &str, hours: u16| {
        format!(r#"{{"id":"{id}","day":"{day}","kind":"{kind}","hours":{hours},"ext":{{}}}}"#)
    };
    let expected = [
        row(&ids[1], "2026-01-05", "regular", 700),
        row(&ids[0], "2026-01-06", "overtime", 200),
        row(&ids[0], "2026-01-06", "regular", 1200),
    ];
    let summed = ok(post("/daily-reports/hours-by-day", "viewer", "j1"));
    assert_eq!(summed, format!("[{}]", expected.join(",")));
    assert_eq!(ok(post("/daily-reports/hours-by-day", "viewer", "j2")), "[]");
    assert_eq!(post("/daily-reports/hours-by-day", "other", "j1").status, 404);
}

#[test]
fn a_task_with_hours_reported_on_it_is_not_archived() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    let post = |path: &str, who: &str, body: &str| send(&openworks, path, who, body);
    let id = reported(post);

    let kept = post("/tasks/archive", "owner", "t1");
    assert_eq!(kept.body, "invalid: task t1 has hours reported on it");
    assert_eq!(ok(post("/tasks/archive", "owner", "t2")), "t2");
    ok(post("/daily-reports/set-hours", "owner", &id));
    assert_eq!(ok(post("/tasks/archive", "owner", "t1")), "t1");
    assert_eq!(host.store().archived, [text("t2"), text("t1")]);
}
