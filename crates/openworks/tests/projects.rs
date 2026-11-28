mod memory;

use std::collections::HashMap;
use std::sync::Barrier;
use std::thread;

use memory::{T0, ok, send};
use openworks::Openworks;
use openworks_core::{Fail, Hours, Id, Role, Row, Tx, Value};
use openworks_db::{
    AUDIT, EVENT, clients, events, jobs, lost_reasons, members, project_members, project_roles, projects, series,
    sites, stages, tasks,
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
    tasks: Vec<Row>,
    numbers: HashMap<String, i64>,
    events: Vec<Value>,
    trail: Vec<Row>,
    refs: Vec<(&'static str, Value, Value, Value)>,
    audits: usize,
}

fn day(value: &Value) -> Option<&str> {
    if let Value::Text(day) = value { Some(day) } else { None }
}

fn int(value: &Value) -> usize {
    if let Value::Int(int) = value {
        usize::try_from(*int).unwrap()
    } else {
        panic!("{value:?}")
    }
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
            _ => self.task_rows(sql, p),
        }
    }

    fn task_rows(&mut self, sql: &'static str, p: &[Value]) -> Vec<Row> {
        match sql {
            tasks::ADD => {
                self.tasks.push([p, &[Value::Null]].concat());
                Vec::new()
            }
            tasks::LIST => {
                let mut rows: Vec<Row> = self
                    .tasks
                    .iter()
                    .filter(|row| row[0] == p[0] && row[2] == p[1])
                    .map(|row| row[1..].to_vec())
                    .collect();
                rows.sort_by_key(|row| format!("{:?}{:?}", row[9], row[0]));
                rows
            }
            tasks::GET => self
                .tasks
                .iter()
                .filter(|row| {
                    row[..2] == p[..2]
                        && self.jobs.iter().any(|job| {
                            job[0] == row[0]
                                && job[1] == row[2]
                                && self.reaches(&vec![job[0].clone(), job[2].clone()], &p[2])
                        })
                })
                .map(|row| row[1..].to_vec())
                .collect(),
            tasks::SAVE => self
                .tasks
                .iter_mut()
                .find(|row| row[..3] == p[..3])
                .map_or_else(Vec::new, |row| {
                    row[3..12].clone_from_slice(&p[3..12]);
                    vec![vec![row[1].clone()]]
                }),
            tasks::ARCHIVE => change(&mut self.tasks, p, |row| {
                if row[12] == Value::Null {
                    row[12] = p[2].clone();
                }
            }),
            tasks::RESTORE => change(&mut self.tasks, p, |row| row[12] = Value::Null),
            _ => Vec::new(),
        }
    }

    fn reads(&self, sql: &'static str, p: &[Value]) -> Option<Vec<Row>> {
        let on = |row: &[Value], at: usize, value: &Value| *value == Value::Null || row[at] == *value;
        let staffed = |job: &Row, person: &Value| self.reaches(&vec![job[0].clone(), job[2].clone()], person);
        Some(match sql {
            projects::LIST => {
                let archived = |row: &Row| on(&[Value::Int((row[15] != Value::Null).into())], 0, &p[4]);
                let kept = self.projects.iter().filter(|row| {
                    row[0] == p[0]
                        && self.reaches(row, &p[1])
                        && on(row, 8, &p[2])
                        && on(row, 3, &p[3])
                        && archived(row)
                });
                let rows = sorted(kept, |row| format!("{:?}{:?}", row[1], row[0]));
                rows.into_iter().skip(int(&p[6])).take(int(&p[5])).collect()
            }
            jobs::BOARD => {
                let meets =
                    |row: &Row| day(&row[8]).is_some() && day(&row[8]) <= day(&p[2]) && day(&row[9]) >= day(&p[1]);
                let kept = self
                    .jobs
                    .iter()
                    .filter(|row| row[0] == p[0] && row[4] != text("cancelled") && meets(row) && staffed(row, &p[3]));
                sorted(kept, |row| format!("{:?}{:?}", row[7], row[0]))
            }
            tasks::BY_PROJECT => {
                let of = |row: &Row| {
                    self.jobs
                        .iter()
                        .any(|job| job[..2] == [row[0].clone(), row[2].clone()] && job[2] == p[1])
                };
                let kept = self.tasks.iter().filter(|row| row[0] == p[0] && of(row));
                sorted(kept, |row| format!("{:?}{:?}{:?}", row[1], row[9], row[0]))
            }
            events::SINCE => self
                .trail
                .iter()
                .zip(1..)
                .filter(|(row, seq)| row[1] == p[0] && *seq > int(&p[1]))
                .take(int(&p[2]))
                .map(|(row, seq)| [&row[..1], &[Value::Int(seq.try_into().unwrap())], &row[2..]].concat())
                .collect(),
            _ => return None,
        })
    }

    fn imports(&mut self, sql: &'static str, p: &[Value]) -> Option<Vec<Row>> {
        let (org, id, blank) = (p[0].clone(), p[1].clone(), text(""));
        let added = match sql {
            clients::IMPORT => vec![(clients::ADD, p[..4].to_vec())],
            sites::IMPORT => vec![(sites::ADD, p[..6].to_vec())],
            projects::IMPORT => vec![(
                projects::ADD,
                [&p[..5], &[blank.clone(), blank.clone(), blank, p[5].clone()]].concat(),
            )],
            jobs::IMPORT => vec![
                (jobs::ADD, p[..3].to_vec()),
                (
                    jobs::MOVE,
                    vec![
                        org.clone(),
                        id.clone(),
                        p[3].clone(),
                        p[4].clone(),
                        Value::Null,
                        Value::Null,
                    ],
                ),
                (jobs::PLAN, vec![org.clone(), id.clone(), p[5].clone(), p[6].clone()]),
            ],
            tasks::IMPORT => vec![(
                tasks::ADD,
                [&p[..5], &[Value::Null], &p[5..8], &[Value::Null, p[8].clone(), blank]].concat(),
            )],
            _ => return None,
        };
        let reference = p.last()?.clone();
        let key = (sql, org, reference);
        if let Some((.., kept)) = self
            .refs
            .iter()
            .find(|(table, org, reference, _)| (*table, org, reference) == (key.0, &key.1, &key.2))
        {
            return Some(vec![vec![kept.clone()]]);
        }
        for (sql, params) in added {
            self.run(sql, &params).ok()?;
        }
        self.refs.push((key.0, key.1, key.2, id.clone()));
        Some(vec![vec![id]])
    }
}

fn sorted<'a>(rows: impl Iterator<Item = &'a Row>, key: impl Fn(&Row) -> String) -> Vec<Row> {
    let mut rows: Vec<Row> = rows.map(|row| row[1..].to_vec()).collect();
    rows.sort_by_key(key);
    rows
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
                self.trail.push(p.to_vec());
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
            _ => match self.reads(sql, p).or_else(|| self.imports(sql, p)) {
                Some(rows) => rows,
                None => self.project_rows(sql, p),
            },
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

fn tasks(openworks: &Openworks<&TestHost>, credential: &str, job: &str) -> String {
    ok(send(openworks, "/tasks/list", credential, job))
}

#[test]
fn a_job_saves_its_tasks_one_or_many_in_order() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    set_up(&openworks);
    let job = job(&openworks, &project(&openworks, "Hall"));
    let audits = host.store().audits;
    let events = host.store().events.len();

    let saved = ok(send(
        &openworks,
        "/tasks/save",
        "owner",
        &format!("{job}\n\tDIG\tDig\t#aa5500\t\t2\t1650\t2500\t\tby hand\n\tPOUR\tPour\t\t2\t\t\t\t\t"),
    ));

    let [dig, pour] = saved.split('\n').collect::<Vec<_>>()[..] else {
        panic!("{saved}")
    };
    assert_eq!(host.store().audits, audits + 1);
    assert_eq!(host.store().events.len(), events + 1);
    assert_eq!(
        tasks(&openworks, "viewer", &job),
        format!(
            r##"[{{"id":"{dig}","job_id":"{job}","code":"DIG","name":"Dig","color":"#aa5500","start_offset":0,"days":2,"planned_hours":1650,"weight":2500,"order":1,"remark":"by hand","archived_at":null,"ext":{{}}}},{{"id":"{pour}","job_id":"{job}","code":"POUR","name":"Pour","color":null,"start_offset":2,"days":1,"planned_hours":null,"weight":null,"order":2,"remark":"","archived_at":null,"ext":{{}}}}]"##
        )
    );

    let again = ok(send(
        &openworks,
        "/tasks/save",
        "owner",
        &format!("{job}\n{pour}\tPOUR\tPour slab\t\t3\t1\t800\t\t0\t\n\tCURE\tCure\t\t4\t3\t\t\t\t"),
    ));

    let cure = again.strip_prefix(&format!("{pour}\n")).unwrap();
    let listed = tasks(&openworks, "owner", &job);
    let order: Vec<&str> = [pour, dig, cure]
        .into_iter()
        .map(|id| &listed[listed.find(id).unwrap()..])
        .collect();
    assert!(order.windows(2).all(|pair| pair[0].len() > pair[1].len()), "{listed}");
    assert!(
        listed.contains(r#""name":"Pour slab","color":null,"start_offset":3,"days":1,"planned_hours":800"#),
        "{listed}"
    );
    assert!(listed.contains(&format!(r#""id":"{cure}","job_id":"{job}","code":"CURE","name":"Cure","color":null,"start_offset":4,"days":3,"planned_hours":null,"weight":null,"order":3"#)), "{listed}");
}

fn bad_lines(job: &str, theirs: &str) -> Vec<(String, String)> {
    [
        ("\tA\tA\tred\t\t\t\t\t\t", "invalid: a colour is #rrggbb, not red"),
        ("\tA\tA\t\t\t0\t\t\t\t", "invalid: a task lasts at least one day"),
        (
            "\tA\tA\t\t-1\t\t\t\t\t",
            "invalid: start_offset is a whole number, not -1",
        ),
        (
            "\tA\tA\t\t\t\t-5\t\t\t",
            "invalid: planned_hours is whole hundredths of an hour, not -5",
        ),
        (
            "\tA\tA\t\t\t\t7.5\t\t\t",
            "invalid: planned_hours is whole hundredths of an hour, not 7.5",
        ),
        ("\tA\t\t\t\t\t\t\t\t", "invalid: a name is needed"),
        ("\tA\tA", "invalid: a task line is 10 tab-separated fields"),
        (
            &format!("{theirs}\tA\tA\t\t\t\t\t\t\t"),
            &format!("invalid: task {theirs} is not on job {job}"),
        ),
    ]
    .into_iter()
    .map(|(line, why)| (line.to_owned(), why.to_owned()))
    .collect()
}

#[test]
fn a_task_line_out_of_shape_saves_nothing() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    set_up(&openworks);
    let job = job(&openworks, &project(&openworks, "Hall"));
    let other = self::job(&openworks, &project(&openworks, "Other"));
    let theirs = ok(send(
        &openworks,
        "/tasks/save",
        "owner",
        &format!("{other}\n\tA\tA\t\t\t\t\t\t\t"),
    ));

    for (line, why) in bad_lines(&job, &theirs) {
        let body = format!("{job}\n\tOK\tFine\t\t\t\t\t\t\t\n{line}");
        assert_eq!(status(&openworks, "/tasks/save", "owner", &body), (400, why), "{line}");
    }
    assert_eq!(tasks(&openworks, "owner", &job), "[]");
    assert_eq!(
        status(
            &openworks,
            "/tasks/save",
            "viewer",
            &format!("{job}\n\tA\tA\t\t\t\t\t\t\t")
        )
        .0,
        403
    );
    assert_eq!(
        status(
            &openworks,
            "/tasks/save",
            "member",
            &format!("{job}\n\tA\tA\t\t\t\t\t\t\t")
        )
        .0,
        404
    );
    assert_eq!(status(&openworks, "/tasks/list", "member", &job).0, 404);
}

#[test]
fn a_task_is_archived_once_and_restored() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    set_up(&openworks);
    let job = job(&openworks, &project(&openworks, "Hall"));
    let task = ok(send(
        &openworks,
        "/tasks/save",
        "owner",
        &format!("{job}\n\tDIG\tDig\t\t\t\t\t\t\t"),
    ));
    let archived = |openworks: &Openworks<&TestHost>| {
        let listed = tasks(openworks, "owner", &job);
        listed[listed.find("archived_at").unwrap() + 13..]
            .split([',', '}'])
            .next()
            .unwrap()
            .to_owned()
    };

    host.set_clock(5);
    assert_eq!(ok(send(&openworks, "/tasks/archive", "owner", &task)), task);
    host.set_clock(9);
    assert_eq!(ok(send(&openworks, "/tasks/archive", "manager", &task)), task);
    assert_eq!(archived(&openworks), (T0 + 5).to_string());

    assert_eq!(status(&openworks, "/tasks/restore", "viewer", &task).0, 403);
    assert_eq!(status(&openworks, "/tasks/restore", "member", &task).0, 404);
    assert_eq!(status(&openworks, "/tasks/archive", "other", &task).0, 404);
    assert_eq!(ok(send(&openworks, "/tasks/restore", "owner", &task)), task);
    assert_eq!(archived(&openworks), "null");
}

#[test]
fn only_a_task_with_no_reported_hours_may_be_archived() {
    assert!(Hours::ZERO.lets_archive());
    assert!(!Hours::hundredths(25).unwrap().lets_archive());
    assert_eq!(Hours::hundredths(-1), None);
}

fn ids(listed: &str) -> Vec<String> {
    let items = listed.split(r#"{"id":""#).skip(1);
    items.map(|item| item.split('"').next().unwrap().to_owned()).collect()
}

#[test]
fn the_project_list_filters_by_stage_client_and_archive_and_pages() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    set_up(&openworks);
    let client = ok(send(&openworks, "/clients/save", "owner", "\nAcme\n"));
    let body = format!("\nAcme hall\n{client}\n\n\n\n\nlead");
    let acme = ok(send(&openworks, "/projects/save", "owner", &body));
    let [cabin, barn] = ["Cabin", "Barn"].map(|name| project(&openworks, name));
    ok(send(
        &openworks,
        "/projects/move-stage",
        "owner",
        &format!("{barn}\nquoted"),
    ));
    ok(send(&openworks, "/projects/archive", "owner", &cabin));
    let listed = |body: &str| ids(&ok(send(&openworks, "/projects/list", "viewer", body)));

    assert_eq!(listed(""), [&acme, &barn, &cabin].map(String::to_owned));
    assert_eq!(listed("quoted"), std::slice::from_ref(&barn));
    assert_eq!(listed(&format!("\n{client}")), std::slice::from_ref(&acme));
    assert_eq!(listed("\n\nyes"), std::slice::from_ref(&cabin));
    assert_eq!(listed("lead\n\nno"), std::slice::from_ref(&acme));
    assert_eq!(listed("\n\n\n2"), [acme, barn]);
    assert_eq!(listed("\n\n\n2\n2"), [cabin]);
    let refused = |body: &str| status(&openworks, "/projects/list", "viewer", body);
    assert_eq!(refused("\n\nmaybe"), (400, "invalid: archived is yes or no".to_owned()));
    assert_eq!(refused("\n\n\n501"), (400, "invalid: limit is 1 to 500".to_owned()));
}

#[test]
fn a_projects_page_holds_its_record_its_people_and_its_jobs() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    set_up(&openworks);
    let (id, job) = planned(&openworks, "Hall");
    ok(send(
        &openworks,
        "/projects/members-save",
        "owner",
        &format!("{id}\nmember lead"),
    ));

    let page = ok(send(&openworks, "/projects/page", "member", &id));

    let got = ok(send(&openworks, "/projects/get", "owner", &id));
    let members = ok(send(&openworks, "/projects/members", "owner", &id));
    let jobs = ok(send(&openworks, "/jobs/list", "owner", &id));
    let project = &got[1..got.len() - 1];
    assert_eq!(
        page,
        format!(r#"{{"project":{project},"members":{members},"jobs":{jobs}}}"#)
    );
    assert_eq!((ids(&members), ids(&jobs)), (vec!["member".to_owned()], vec![job]));
    assert_eq!(send(&openworks, "/projects/page", "member2", &id).status, 404);
    assert_eq!(send(&openworks, "/projects/page", "other", &id).status, 404);
}

#[test]
fn the_board_lists_the_jobs_whose_planned_window_meets_the_days() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    set_up(&openworks);
    let (hall, early) = planned(&openworks, "Hall");
    let (_, late) = planned(&openworks, "Barn");
    ok(send(
        &openworks,
        "/jobs/reschedule",
        "owner",
        &format!("{late}\n2026-02-02\n2026-02-06"),
    ));
    project(&openworks, "Shed");
    ok(send(
        &openworks,
        "/projects/members-save",
        "owner",
        &format!("{hall}\nmember lead"),
    ));
    let board = |credential: &str, body: &str| ids(&ok(send(&openworks, "/jobs/board", credential, body)));

    assert_eq!(board("viewer", "2026-01-01\n2026-01-05"), std::slice::from_ref(&early));
    assert_eq!(board("viewer", "2026-01-09\n2026-02-02"), [early.clone(), late.clone()]);
    assert_eq!(board("viewer", "2026-01-10\n2026-02-01"), Vec::<String>::new());
    assert_eq!(board("member", "2026-01-01\n2026-12-31"), std::slice::from_ref(&early));
    assert_eq!(board("other", "2026-01-01\n2026-12-31"), Vec::<String>::new());
    assert_eq!(win(&openworks, &hall).0, 200);
    ok(send(&openworks, "/jobs/cancel", "owner", &early));
    assert_eq!(board("viewer", "2026-01-01\n2026-12-31"), [late]);
    let refused = status(&openworks, "/jobs/board", "viewer", "2026-01-09\n2026-01-05");
    assert_eq!(refused, (400, "invalid: 2026-01-05 is before 2026-01-09".to_owned()));
}

#[test]
fn changes_lists_the_events_after_a_sequence_number_to_a_reader_of_every_project() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    set_up(&openworks);
    let changes = |body: &str| ok(send(&openworks, "/projects/changes", "viewer", body));

    assert_eq!(ids(&changes("")).len(), 8);
    let page = changes("3\n2");
    assert_eq!(ids(&page).len(), 2);
    let fourth = r#""seq":4,"kind":"projects.stage-save","body":"done\u000aDone\u000adone\u000a4","at":1700000000000"#;
    assert!(page.contains(fourth) && page.contains(r#""seq":5,"#), "{page}");
    ok(send(&openworks, "/projects/stage-save", "other", "lead\nLead\nopen\n1"));
    assert_eq!(changes("8"), "[]");
    project(&openworks, "Hall");
    assert!(changes("8").contains(r#""seq":10,"kind":"projects.save""#));
    let refused = status(&openworks, "/projects/changes", "member", "");
    assert_eq!(refused, (403, "needs read".to_owned()));
}

const SPINE: &str = "client\tC1\tAcme
site\tS1\tC1\tYard\t1 Main Road\t
project\tP1\tHall\tC1\tS1\tlead
project\tP2\tBarn\t\t\tlead
job\tJ1\tP1\topen\tOLD-7\t2026-01-05\t2026-01-09
job\tJ2\tP1\tproposed\t\t\t
job\tJ3\tP2\tproposed\t\t\t
task\tT1\tJ1\tDIG\tDig\t0\t2\t1600
task\tT2\tJ1\tPOUR\tPour\t2\t\t
task\tT3\tJ3\tROOF\tRoof\t\t\t";

fn import(openworks: &Openworks<&TestHost>, records: &str) -> Result<String, Fail> {
    openworks.import(Id("host".to_owned()), Id("o1".to_owned()), records)
}

fn kept(store: &Store) -> [usize; 5] {
    [
        store.clients.len(),
        store.sites.len(),
        store.projects.len(),
        store.jobs.len(),
        store.tasks.len(),
    ]
}

#[test]
fn an_import_by_host_ref_run_twice_writes_once() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    set_up(&openworks);

    let once = import(&openworks, SPINE).unwrap();
    let audits = host.store().audits;
    let twice = import(&openworks, SPINE).unwrap();

    assert_eq!(twice, once);
    assert_eq!(kept(&host.store()), [1, 1, 2, 3, 3]);
    assert_eq!(host.store().audits, audits + 1);
    assert_eq!(host.store().events.last(), Some(&text("projects.import")));
    let id = |line: &str| {
        once.lines()
            .find(|kept| kept.starts_with(line))
            .unwrap()
            .rsplit('\t')
            .next()
            .unwrap()
    };
    let (hall, job) = (id("project\tP1\t"), id("job\tJ1\t"));
    assert_eq!(
        field(&openworks, "viewer", hall, "site_id"),
        format!(r#""{}""#, id("site\tS1\t"))
    );
    assert_eq!(job_field(&openworks, job, "number"), r#""OLD-7""#);
    assert_eq!(
        ids(&ok(send(&openworks, "/jobs/board", "viewer", "2026-01-09\n2026-01-09"))),
        [job]
    );
    let tasks = ok(send(&openworks, "/tasks/by-project", "viewer", hall));
    assert_eq!(ids(&tasks), [id("task\tT1\t"), id("task\tT2\t")]);
    assert!(tasks.contains(r#""code":"POUR","name":"Pour","color":null,"start_offset":2,"days":1,"planned_hours":null,"weight":null,"order":2"#), "{tasks}");
    assert_eq!(send(&openworks, "/tasks/by-project", "member", hall).status, 404);
}

#[test]
fn an_import_line_out_of_shape_imports_nothing() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    set_up(&openworks);
    let audits = host.store().audits;

    for (line, why) in [
        ("site\tS1\tC9\tYard\t\t", "no client C9 before this line"),
        ("project\tP2\tHall\tC1\tS9\tlead", "site S9 is not one of client C1's"),
        (
            "job\tJ1\tP1\topen\t\t\t",
            "a job has a number once it is no longer proposed",
        ),
        ("job\tJ1\tP1\tpaused\t\t\t", "no job status paused"),
        ("task\tT1\tJ9\t\tDig\t\t\t", "no job J9 before this line"),
        ("widget\tW1\tGear", "an import line of kind widget is out of shape"),
    ] {
        let records = format!("client\tC1\tAcme\nproject\tP1\tHall\tC1\t\tlead\n{line}");
        assert_eq!(
            import(&openworks, &records),
            Err(Fail::Invalid(why.to_owned())),
            "{line}"
        );
    }
    assert_eq!(
        import(&openworks, "project\tP1\tHall\t\t\tnowhere"),
        Err(Fail::NotFound)
    );
    assert_eq!(kept(&host.store()), [0; 5]);
    assert_eq!(host.store().audits, audits);
    assert_eq!(send(&openworks, "/projects/import", "manager", SPINE).status, 403);
}
