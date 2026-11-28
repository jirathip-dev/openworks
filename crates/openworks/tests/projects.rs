mod memory;

use std::collections::HashMap;
use std::sync::Barrier;
use std::thread;

use memory::{T0, ok, send};
use openworks::Openworks;
use openworks_core::{Fail, Role, Row, Tx, Value};
use openworks_db::{
    AUDIT, EVENT, clients, jobs, lost_reasons, members, project_members, project_roles, projects, series, sites, stages,
};

#[derive(Clone, Default)]
struct Store {
    stages: Vec<Row>,
    reasons: Vec<Row>,
    roles: Vec<Row>,
    clients: Vec<Row>,
    sites: Vec<Row>,
    projects: Vec<Row>,
    staffed: Vec<Row>,
    jobs: Vec<Row>,
    numbers: HashMap<String, i64>,
    events: Vec<Value>,
    audits: usize,
}

fn text(value: &str) -> Value {
    Value::Text(value.to_owned())
}

fn upsert(table: &mut Vec<Row>, row: &[Value]) {
    match table.iter_mut().find(|kept| kept[..2] == row[..2]) {
        Some(kept) => kept.clone_from_slice(row),
        None => table.push(row.to_vec()),
    }
}

fn of_org(table: &[Row], org: &Value) -> Vec<Row> {
    table
        .iter()
        .filter(|row| row[0] == *org)
        .map(|row| row[1..].to_vec())
        .collect()
}

fn keyed(table: &[Row], p: &[Value]) -> Vec<Row> {
    table
        .iter()
        .filter(|row| row[..2] == p[..2])
        .map(|row| row[1..].to_vec())
        .collect()
}

fn change(table: &mut [Row], p: &[Value], set: impl FnOnce(&mut Row)) -> Vec<Row> {
    table
        .iter_mut()
        .find(|row| row[..2] == p[..2])
        .map_or_else(Vec::new, |row| {
            set(row);
            vec![vec![row[1].clone()]]
        })
}

impl Store {
    fn reaches(&self, project: &Row, person: &Value) -> bool {
        *person == Value::Null
            || self
                .staffed
                .iter()
                .any(|row| row[..2] == project[..2] && row[2] == *person)
    }

    fn project_rows(&mut self, sql: &'static str, p: &[Value]) -> Vec<Row> {
        match sql {
            projects::ADD => {
                let mut row = p.to_vec();
                row.extend([
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    text(""),
                    Value::Null,
                ]);
                self.projects.push(row);
                Vec::new()
            }
            projects::LIST => self
                .projects
                .iter()
                .filter(|row| row[0] == p[0] && self.reaches(row, &p[1]))
                .map(|row| row[1..].to_vec())
                .collect(),
            projects::GET => self
                .projects
                .iter()
                .filter(|row| row[..2] == p[..2] && self.reaches(row, &p[2]))
                .map(|row| row[1..].to_vec())
                .collect(),
            projects::SAVE => change(&mut self.projects, p, |row| row[2..8].clone_from_slice(&p[2..8])),
            projects::MOVE => change(&mut self.projects, p, |row| {
                row[8] = p[2].clone();
                row[11..15].clone_from_slice(&p[3..7]);
            }),
            projects::LINK => change(&mut self.projects, p, |row| row[9..11].clone_from_slice(&p[2..4])),
            projects::ARCHIVE => change(&mut self.projects, p, |row| {
                if row[15] == Value::Null {
                    row[15] = p[2].clone();
                }
            }),
            project_members::CLEAR => {
                self.staffed.retain(|row| row[..2] != p[..2]);
                Vec::new()
            }
            project_members::ADD => {
                if !self.staffed.iter().any(|row| row[..] == p[..]) {
                    self.staffed.push(p.to_vec());
                }
                Vec::new()
            }
            project_members::LIST => keyed(&self.staffed, p)
                .into_iter()
                .map(|row| row[1..].to_vec())
                .collect(),
            _ => self.job_rows(sql, p),
        }
    }

    fn job_rows(&mut self, sql: &'static str, p: &[Value]) -> Vec<Row> {
        match sql {
            jobs::ADD => {
                let mut row = p.to_vec();
                row.extend([Value::Null, text("proposed"), Value::Int(1)]);
                row.resize(row.len() + 6, Value::Null);
                row.extend([text(""), Value::Null]);
                self.jobs.push(row);
                Vec::new()
            }
            jobs::LIST => self
                .jobs
                .iter()
                .filter(|row| row[0] == p[0] && row[2] == p[1])
                .map(|row| row[1..].to_vec())
                .collect(),
            jobs::GET => self
                .jobs
                .iter()
                .filter(|row| row[..2] == p[..2] && self.reaches(&vec![row[0].clone(), row[2].clone()], &p[2]))
                .map(|row| row[1..].to_vec())
                .collect(),
            jobs::SAVE => change(&mut self.jobs, p, |row| {
                row[5..8].clone_from_slice(&p[2..5]);
                row[10] = p[5].clone();
                row[12] = p[6].clone();
            }),
            jobs::PLAN => change(&mut self.jobs, p, |row| row[8..10].clone_from_slice(&p[2..4])),
            jobs::MOVE => change(&mut self.jobs, p, |row| {
                row[4] = p[2].clone();
                row[3] = p[3].clone();
                row[11] = p[4].clone();
                row[13] = p[5].clone();
            }),
            series::NEXT => {
                let last = self.numbers.entry(format!("{p:?}")).or_insert(0);
                *last += 1;
                vec![vec![Value::Int(*last)]]
            }
            _ => Vec::new(),
        }
    }
}

impl Tx for Store {
    fn run(&mut self, sql: &'static str, p: &[Value]) -> Result<Vec<Row>, Fail> {
        Ok(match sql {
            members::FIND => match &p[1] {
                Value::Text(person) if person == "stranger" => Vec::new(),
                Value::Text(person) if person == "viewer" => vec![vec![text(Role::Viewer.name())]],
                Value::Text(person) if person == "manager" => vec![vec![text(Role::Manager.name())]],
                Value::Text(person) if person.starts_with("member") => vec![vec![text(Role::Member.name())]],
                _ => vec![vec![text(Role::Owner.name())]],
            },
            AUDIT => {
                self.audits += 1;
                Vec::new()
            }
            EVENT => {
                self.events.push(p[2].clone());
                Vec::new()
            }
            stages::SAVE => {
                upsert(&mut self.stages, p);
                Vec::new()
            }
            stages::GET => keyed(&self.stages, p),
            stages::LIST => {
                let mut rows = of_org(&self.stages, &p[0]);
                rows.sort_by_key(|row| format!("{:?}", row[3]));
                rows
            }
            lost_reasons::LIST => of_org(&self.reasons, &p[0]),
            lost_reasons::SAVE => {
                upsert(&mut self.reasons, p);
                Vec::new()
            }
            lost_reasons::GET => keyed(&self.reasons, p),
            project_roles::SAVE => {
                upsert(&mut self.roles, p);
                Vec::new()
            }
            project_roles::GET => keyed(&self.roles, p),
            clients::ADD => {
                self.clients.push([&p[..4], &[Value::Null]].concat());
                Vec::new()
            }
            clients::GET => keyed(&self.clients, p),
            sites::ADD => {
                self.sites.push([&p[..6], &[Value::Null]].concat());
                Vec::new()
            }
            sites::GET => keyed(&self.sites, p),
            _ => self.project_rows(sql, p),
        })
    }
}

type TestHost = memory::TestHost<Store>;

fn status(openworks: &Openworks<&TestHost>, path: &str, credential: &str, body: &str) -> (u16, String) {
    let response = send(openworks, path, credential, body);
    (response.status, response.body)
}

fn set_up(openworks: &Openworks<&TestHost>) {
    for stage in [
        "lead\nLead\nopen\n1",
        "quoted\nQuoted\nopen\n2",
        "won\nWon\nwon\n3",
        "done\nDone\ndone\n4",
        "lost\nLost\nlost\n5",
    ] {
        ok(send(openworks, "/projects/stage-save", "owner", stage));
    }
    ok(send(
        openworks,
        "/projects/lost-reason-save",
        "owner",
        "late\nToo late\nno",
    ));
    ok(send(
        openworks,
        "/projects/lost-reason-save",
        "owner",
        "other\nOther\nyes",
    ));
    ok(send(openworks, "/projects/role-save", "owner", "lead\nLead"));
}

fn project(openworks: &Openworks<&TestHost>, name: &str) -> String {
    ok(send(
        openworks,
        "/projects/save",
        "owner",
        &format!("\n{name}\n\n\n\n\n\nlead"),
    ))
}

fn field(openworks: &Openworks<&TestHost>, credential: &str, id: &str, name: &str) -> String {
    read(openworks, "/projects/get", credential, id, name)
}

fn read(openworks: &Openworks<&TestHost>, path: &str, credential: &str, id: &str, name: &str) -> String {
    let got = ok(send(openworks, path, credential, id));
    let at = got.find(&format!("\"{name}\":")).unwrap() + name.len() + 3;
    got[at..].split([',', '}']).next().unwrap().to_owned()
}

#[test]
fn the_company_keeps_its_stages_with_a_kind_lost_reasons_and_roles() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    set_up(&openworks);
    ok(send(
        &openworks,
        "/projects/stage-save",
        "owner",
        "lead\nNew lead\nopen\n1",
    ));

    let listed = ok(send(&openworks, "/projects/stages", "viewer", ""));
    assert!(
        listed.starts_with(r#"[{"id":"lead","name":"New lead","kind":"open","sort":1,"ext":{}},{"id":"quoted""#),
        "{listed}"
    );
    assert_eq!(
        ok(send(&openworks, "/projects/lost-reasons", "viewer", "")),
        r#"[{"id":"late","name":"Too late","needs_note":0,"ext":{}},{"id":"other","name":"Other","needs_note":1,"ext":{}}]"#
    );
    assert_eq!(
        status(&openworks, "/projects/stage-save", "owner", "x\nX\nmaybe"),
        (400, "invalid: no stage kind maybe".to_owned())
    );
    assert_eq!(
        status(&openworks, "/projects/stage-save", "manager", "x\nX\nopen").0,
        403
    );
    assert_eq!(
        status(&openworks, "/projects/lost-reason-save", "owner", "x\nX\nperhaps").0,
        400
    );
    assert_eq!(host.store().audits, 9);
}

#[test]
fn a_project_holds_its_name_client_site_scope_tags_and_zone() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    set_up(&openworks);
    let client = ok(send(&openworks, "/clients/save", "owner", "\nAcme"));
    let other = ok(send(&openworks, "/clients/save", "owner", "\nBeta"));
    let site = ok(send(&openworks, "/sites/save", "owner", &format!("\n{client}\nYard")));

    let id = ok(send(
        &openworks,
        "/projects/save",
        "owner",
        &format!("\nNew hall\n{client}\n{site}\nTwo floors\nnorth, big ,north\nZone A\nlead"),
    ));

    assert_eq!(
        ok(send(&openworks, "/projects/get", "viewer", &id)),
        format!(
            r#"[{{"id":"{id}","name":"New hall","client_id":"{client}","site_id":"{site}","scope":"Two floors","tags":"north,big","zone":"Zone A","stage":"lead","parent_id":null,"link_kind":null,"won_at":null,"lost_at":null,"lost_reason":null,"lost_note":"","archived_at":null,"ext":{{}}}}]"#
        )
    );
    ok(send(
        &openworks,
        "/projects/save",
        "owner",
        &format!("{id}\nOld hall\n{client}"),
    ));
    assert_eq!(field(&openworks, "owner", &id, "name"), r#""Old hall""#);
    assert_eq!(field(&openworks, "owner", &id, "site_id"), "null");
    assert_eq!(
        status(
            &openworks,
            "/projects/save",
            "owner",
            &format!("\nX\n{other}\n{site}\n\n\n\nlead")
        ),
        (400, "invalid: a project's site is one of its client's".to_owned())
    );
    assert_eq!(
        status(&openworks, "/projects/save", "owner", "\nX\n\n\n\n\n\nwon"),
        (400, "invalid: a project starts at an open stage".to_owned())
    );
    assert_eq!(
        status(
            &openworks,
            "/projects/save",
            "owner",
            &format!("{id}\nX\n\n\n\n\n\nwon")
        ),
        (400, "invalid: a project's stage changes by move-stage".to_owned())
    );
    assert_eq!(
        status(&openworks, "/projects/save", "owner", "\nX\nnobody\n\n\n\n\nlead").0,
        404
    );
    assert_eq!(
        status(&openworks, "/projects/save", "viewer", "\nX\n\n\n\n\n\nlead").0,
        403
    );
    assert_eq!(ok(send(&openworks, "/projects/list", "other", "")), "[]");
    let events = &host.store().events;
    assert_eq!(events.last(), Some(&text("projects.save")));
}

#[test]
fn a_stage_move_follows_the_rules_of_its_kinds() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    set_up(&openworks);
    let (id, job) = planned(&openworks, "Hall");
    let moving = |body: &str| status(&openworks, "/projects/move-stage", "owner", &format!("{id}\n{body}"));

    assert_eq!(moving("quoted").0, 200);
    assert_eq!(
        moving("done"),
        (
            400,
            "invalid: a project at a open stage does not move to a done stage".to_owned()
        )
    );
    host.set_clock(1);
    assert_eq!(moving("won").0, 200);
    assert_eq!(field(&openworks, "owner", &id, "won_at"), (T0 + 1).to_string());
    assert_eq!(moving("lost\nlate").0, 400);
    ok(send(&openworks, "/jobs/cancel", "owner", &job));
    assert_eq!(moving("done").0, 200);
    assert_eq!(moving("won").0, 200);
    assert_eq!(field(&openworks, "owner", &id, "won_at"), (T0 + 1).to_string());
    assert_eq!(moving("lead").0, 200);
    assert_eq!(field(&openworks, "owner", &id, "won_at"), "null");

    assert_eq!(
        moving("lost"),
        (400, "invalid: a lost project needs a reason".to_owned())
    );
    assert_eq!(
        moving("lost\nother"),
        (400, "invalid: the reason other needs a note".to_owned())
    );
    assert_eq!(moving("lost\nnone").0, 404);
    assert_eq!(
        moving("quoted\nlate"),
        (400, "invalid: only a lost project has a reason".to_owned())
    );
    assert_eq!(moving("lost\nother\nWent elsewhere").0, 200);
    assert_eq!(field(&openworks, "owner", &id, "lost_reason"), r#""other""#);
    assert_eq!(field(&openworks, "owner", &id, "lost_note"), r#""Went elsewhere""#);
    assert_eq!(moving("won").0, 400);
    assert_eq!(moving("lead").0, 200);
    assert_eq!(field(&openworks, "owner", &id, "lost_reason"), "null");
    assert_eq!(field(&openworks, "owner", &id, "lost_at"), "null");
    assert_eq!(moving("nowhere").0, 404);
    assert_eq!(host.store().events.last(), Some(&text("projects.move-stage")));
}

#[test]
fn own_scope_reads_and_changes_only_the_callers_projects() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    set_up(&openworks);
    let theirs = project(&openworks, "Theirs");
    let other = project(&openworks, "Other");

    ok(send(
        &openworks,
        "/projects/members-save",
        "owner",
        &format!("{theirs}\nmember lead\nmember lead"),
    ));

    let listed = ok(send(&openworks, "/projects/list", "member", ""));
    assert!(listed.contains(&theirs) && !listed.contains(&other), "{listed}");
    assert_eq!(ok(send(&openworks, "/projects/list", "member2", "")), "[]");
    assert_eq!(send(&openworks, "/projects/get", "member", &other).status, 404);
    assert_eq!(
        send(
            &openworks,
            "/projects/move-stage",
            "member",
            &format!("{other}\nquoted")
        )
        .status,
        404
    );
    assert_eq!(
        send(
            &openworks,
            "/projects/move-stage",
            "member",
            &format!("{theirs}\nquoted")
        )
        .status,
        200
    );
    assert!(ok(send(&openworks, "/projects/list", "viewer", "")).contains(&other));
    assert_eq!(
        ok(send(&openworks, "/projects/members", "member", &theirs)),
        r#"[{"id":"member","role":"lead","ext":{}}]"#
    );
    assert_eq!(
        status(
            &openworks,
            "/projects/members-save",
            "owner",
            &format!("{theirs}\nstranger lead")
        ),
        (400, "invalid: stranger is not a member of the company".to_owned())
    );
    assert_eq!(
        status(
            &openworks,
            "/projects/members-save",
            "owner",
            &format!("{theirs}\nmember boss")
        ),
        (400, "invalid: no project role boss".to_owned())
    );
    ok(send(&openworks, "/projects/members-save", "owner", &theirs));
    assert_eq!(ok(send(&openworks, "/projects/list", "member", "")), "[]");
}

#[test]
fn a_project_links_to_a_parent_with_a_kind_and_never_under_itself() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    set_up(&openworks);
    let main = project(&openworks, "Main");
    let extra = project(&openworks, "Extra");
    let linking = |body: &str| status(&openworks, "/projects/link", "owner", body);

    assert_eq!(linking(&format!("{extra}\n{main}\nvariation")).0, 200);
    assert_eq!(field(&openworks, "owner", &extra, "parent_id"), format!("\"{main}\""));
    assert_eq!(field(&openworks, "owner", &extra, "link_kind"), r#""variation""#);
    assert_eq!(
        linking(&format!("{main}\n{extra}\nvariation")),
        (400, "invalid: a project cannot sit under itself".to_owned())
    );
    assert_eq!(
        linking(&format!("{main}\n{main}\nvariation")).1,
        "invalid: a project cannot sit under itself"
    );
    assert_eq!(
        linking(&format!("{main}\n{extra}")),
        (400, "invalid: a link is a parent and a kind".to_owned())
    );
    assert_eq!(linking(&format!("{main}\nnobody\nvariation")).0, 404);
    assert_eq!(linking(&extra).0, 200);
    assert_eq!(field(&openworks, "owner", &extra, "parent_id"), "null");
    assert_eq!(field(&openworks, "owner", &extra, "link_kind"), "null");
}

#[test]
fn a_project_is_archived_once_and_keeps_its_first_time() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    set_up(&openworks);
    let id = project(&openworks, "Hall");

    ok(send(&openworks, "/projects/archive", "owner", &id));
    host.set_clock(5);
    ok(send(&openworks, "/projects/archive", "owner", &id));

    assert_eq!(field(&openworks, "owner", &id, "archived_at"), T0.to_string());
    assert_eq!(send(&openworks, "/projects/archive", "other", &id).status, 404);
    assert_eq!(send(&openworks, "/projects/archive", "viewer", &id).status, 403);
    assert_eq!(host.store().events.last(), Some(&text("projects.archive")));
}

fn job(openworks: &Openworks<&TestHost>, project: &str) -> String {
    let listed = ok(send(openworks, "/jobs/list", "owner", project));
    listed.split('"').nth(3).unwrap().to_owned()
}

fn job_field(openworks: &Openworks<&TestHost>, id: &str, name: &str) -> String {
    read(openworks, "/jobs/get", "owner", id, name)
}

fn planned(openworks: &Openworks<&TestHost>, name: &str) -> (String, String) {
    let id = project(openworks, name);
    let job = job(openworks, &id);
    ok(send(
        openworks,
        "/jobs/reschedule",
        "owner",
        &format!("{job}\n2026-01-05\n2026-01-09"),
    ));
    (id, job)
}

fn win(openworks: &Openworks<&TestHost>, id: &str) -> (u16, String) {
    status(openworks, "/projects/move-stage", "owner", &format!("{id}\nwon"))
}

#[test]
fn a_project_is_made_with_its_first_job_proposed() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    set_up(&openworks);
    let audits = host.store().audits;

    let id = project(&openworks, "Hall");

    let job = job(&openworks, &id);
    assert_eq!(
        ok(send(&openworks, "/jobs/list", "viewer", &id)),
        format!(
            r#"[{{"id":"{job}","project_id":"{id}","number":null,"status":"proposed","at_site":1,"base_date":null,"work_days":null,"planned_start":null,"planned_end":null,"actual_start":null,"actual_end":null,"remark":"","finished_at":null,"ext":{{}}}}]"#
        )
    );
    assert_eq!(host.store().audits, audits + 1);
    assert_eq!(send(&openworks, "/jobs/list", "member", &id).status, 404);
    assert_eq!(send(&openworks, "/jobs/get", "member", &job).status, 404);
    assert_eq!(send(&openworks, "/jobs/get", "other", &job).status, 404);
    ok(send(
        &openworks,
        "/projects/members-save",
        "owner",
        &format!("{id}\nmember lead"),
    ));
    assert_eq!(job_field(&openworks, &job, "status"), r#""proposed""#);
    assert_eq!(send(&openworks, "/jobs/get", "member", &job).status, 200);
}

#[test]
fn a_job_keeps_its_planned_window_and_calendar() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    set_up(&openworks);
    let (_, job) = planned(&openworks, "Hall");
    let saving = |body: &str| status(&openworks, "/jobs/save", "owner", &format!("{job}\n{body}"));

    assert_eq!(saving("no\n2026-01-05\n5,1,2,3, 4\n\nGate code 12").0, 200);

    assert_eq!(job_field(&openworks, &job, "planned_start"), r#""2026-01-05""#);
    assert_eq!(job_field(&openworks, &job, "planned_end"), r#""2026-01-09""#);
    assert_eq!(job_field(&openworks, &job, "at_site"), "0");
    assert_eq!(job_field(&openworks, &job, "base_date"), r#""2026-01-05""#);
    let got = ok(send(&openworks, "/jobs/get", "owner", &job));
    assert!(got.contains(r#""work_days":"1,2,3,4,5""#), "{got}");
    assert_eq!(job_field(&openworks, &job, "remark"), r#""Gate code 12""#);
    assert_eq!(saving("maybe"), (400, "invalid: at_site is yes or no".to_owned()));
    assert_eq!(saving("yes\n\n1,8"), (400, "invalid: not ISO weekdays: 1,8".to_owned()));
    assert_eq!(
        saving("yes\n2026-02-29"),
        (400, "invalid: not a YYYY-MM-DD day: 2026-02-29".to_owned())
    );
    let rescheduling = |body: &str| status(&openworks, "/jobs/reschedule", "owner", &format!("{job}\n{body}"));
    assert_eq!(
        rescheduling("2026-01-09\n2026-01-05"),
        (400, "invalid: 2026-01-05 is before 2026-01-09".to_owned())
    );
    assert_eq!(rescheduling("2026-01-05").1, "invalid: not a YYYY-MM-DD day: ");
    assert_eq!(
        status(
            &openworks,
            "/jobs/reschedule",
            "viewer",
            &format!("{job}\n2026-01-05\n2026-01-06")
        )
        .0,
        403
    );
    assert_eq!(host.store().events.last(), Some(&text("jobs.save")));
}

#[test]
fn winning_numbers_the_jobs_and_refuses_one_with_no_window_unless_it_is_workshop_work() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    set_up(&openworks);
    let hall = project(&openworks, "Hall");
    let hall_job = job(&openworks, &hall);

    assert_eq!(
        win(&openworks, &hall),
        (
            400,
            format!("invalid: job {hall_job} needs a planned window unless it is workshop work")
        )
    );
    assert_eq!(field(&openworks, "owner", &hall, "stage"), r#""lead""#);
    assert_eq!(job_field(&openworks, &hall_job, "number"), "null");

    let shop = project(&openworks, "Shop");
    let shop_job = job(&openworks, &shop);
    ok(send(&openworks, "/jobs/save", "owner", &format!("{shop_job}\nno")));
    assert_eq!(win(&openworks, &shop).0, 200);
    assert_eq!(job_field(&openworks, &shop_job, "status"), r#""open""#);
    assert_eq!(job_field(&openworks, &shop_job, "number"), r#""J-2023-1""#);

    ok(send(
        &openworks,
        "/projects/move-stage",
        "owner",
        &format!("{shop}\nlead"),
    ));
    assert_eq!(job_field(&openworks, &shop_job, "status"), r#""proposed""#);
    assert_eq!(job_field(&openworks, &shop_job, "number"), r#""J-2023-1""#);
    assert_eq!(win(&openworks, &shop).0, 200);
    assert_eq!(job_field(&openworks, &shop_job, "number"), r#""J-2023-1""#);

    let (yard, yard_job) = planned(&openworks, "Yard");
    assert_eq!(win(&openworks, &yard).0, 200);
    assert_eq!(job_field(&openworks, &yard_job, "number"), r#""J-2023-2""#);
}

#[test]
fn two_projects_won_at_once_get_two_numbers() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    set_up(&openworks);
    let won = [planned(&openworks, "Hall"), planned(&openworks, "Yard")];
    let start = Barrier::new(2);

    thread::scope(|scope| {
        let winners: Vec<_> = won
            .iter()
            .map(|(id, _)| {
                scope.spawn(|| {
                    let openworks = Openworks::new(&host).build().unwrap();
                    start.wait();
                    win(&openworks, id)
                })
            })
            .collect();
        for winner in winners {
            assert_eq!(winner.join().unwrap().0, 200);
        }
    });

    let mut numbers = won.map(|(_, job)| job_field(&openworks, &job, "number"));
    numbers.sort();
    assert_eq!(numbers, [r#""J-2023-1""#, r#""J-2023-2""#]);
}

#[test]
fn a_job_follows_its_statechart() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    set_up(&openworks);
    let (id, job) = planned(&openworks, "Hall");
    let doing =
        |step: &str, body: &str| status(&openworks, &format!("/jobs/{step}"), "owner", &format!("{job}\n{body}"));
    let refused = |state: &str, step: &str| (400, format!("invalid: a job that is {state} cannot {step}"));

    assert_eq!(doing("finish", "2026-01-09"), refused("proposed", "finish"));
    assert_eq!(doing("cancel", ""), refused("proposed", "cancel"));
    assert_eq!(doing("reopen", ""), refused("proposed", "reopen"));
    assert_eq!(
        doing("save", "yes\n\n\n2026-01-05"),
        refused("proposed", "record actual dates")
    );

    assert_eq!(doing("open", ""), (200, "J-2023-1".to_owned()));
    assert_eq!(field(&openworks, "owner", &id, "stage"), r#""lead""#);
    assert_eq!(doing("open", ""), refused("open", "open"));
    assert_eq!(
        doing("finish", "2026-01-09"),
        (
            400,
            "invalid: a job records its actual start before it finishes".to_owned()
        )
    );
    assert_eq!(doing("save", "yes\n\n\n2026-01-06").0, 200);
    assert_eq!(doing("reschedule", "2026-01-06\n2026-01-12").0, 200);
    assert_eq!(
        doing("finish", "2026-01-05"),
        (400, "invalid: 2026-01-05 is before 2026-01-06".to_owned())
    );
    host.set_clock(7);
    assert_eq!(doing("finish", "2026-01-09").0, 200);
    assert_eq!(job_field(&openworks, &job, "status"), r#""finished""#);
    assert_eq!(job_field(&openworks, &job, "actual_end"), r#""2026-01-09""#);
    assert_eq!(job_field(&openworks, &job, "finished_at"), (T0 + 7).to_string());
}

#[test]
fn a_finished_job_reopens_and_an_open_one_cancels_for_good() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    set_up(&openworks);
    let (_, job) = planned(&openworks, "Hall");
    let doing =
        |step: &str, body: &str| status(&openworks, &format!("/jobs/{step}"), "owner", &format!("{job}\n{body}"));
    let refused = |state: &str, step: &str| (400, format!("invalid: a job that is {state} cannot {step}"));
    ok(send(&openworks, "/jobs/open", "owner", &job));
    assert_eq!(doing("save", "yes\n\n\n2026-01-06").0, 200);
    assert_eq!(doing("finish", "2026-01-09").0, 200);

    assert_eq!(doing("cancel", ""), refused("finished", "cancel"));
    assert_eq!(
        doing("reschedule", "2026-01-06\n2026-01-12"),
        refused("finished", "reschedule")
    );
    assert_eq!(
        doing("save", "yes\n\n\n2026-01-07"),
        refused("finished", "record actual dates")
    );
    assert_eq!(doing("reopen", "").0, 200);
    assert_eq!(job_field(&openworks, &job, "status"), r#""open""#);
    assert_eq!(job_field(&openworks, &job, "actual_end"), "null");
    assert_eq!(job_field(&openworks, &job, "finished_at"), "null");

    assert_eq!(doing("cancel", "").0, 200);
    assert_eq!(job_field(&openworks, &job, "status"), r#""cancelled""#);
    assert_eq!(job_field(&openworks, &job, "number"), r#""J-2023-1""#);
    for step in ["open", "reopen", "finish", "cancel", "reschedule"] {
        assert_eq!(doing(step, "").0, 400, "{step}");
    }
    assert_eq!(status(&openworks, "/jobs/cancel", "viewer", &job).0, 403);
    assert_eq!(status(&openworks, "/jobs/cancel", "member", &job).0, 404);
    assert_eq!(host.store().events.last(), Some(&text("jobs.cancel")));
}

#[test]
fn a_project_is_done_once_every_job_is_finished_or_cancelled_and_reopening_one_wins_it_again() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    set_up(&openworks);
    let (id, job) = planned(&openworks, "Hall");
    let moving = |stage: &str| status(&openworks, "/projects/move-stage", "owner", &format!("{id}\n{stage}"));
    host.set_clock(3);
    assert_eq!(moving("won").0, 200);

    assert_eq!(
        moving("done"),
        (
            400,
            "invalid: a project is done once every job is finished or cancelled".to_owned()
        )
    );
    ok(send(
        &openworks,
        "/jobs/save",
        "owner",
        &format!("{job}\nyes\n\n\n2026-01-05"),
    ));
    ok(send(&openworks, "/jobs/finish", "owner", &format!("{job}\n2026-01-09")));
    assert_eq!(moving("done").0, 200);

    ok(send(&openworks, "/jobs/reopen", "owner", &job));
    assert_eq!(field(&openworks, "owner", &id, "stage"), r#""won""#);
    assert_eq!(field(&openworks, "owner", &id, "won_at"), (T0 + 3).to_string());
    assert_eq!(job_field(&openworks, &job, "status"), r#""open""#);
    ok(send(&openworks, "/jobs/cancel", "owner", &job));
    assert_eq!(moving("done").0, 200);
}
