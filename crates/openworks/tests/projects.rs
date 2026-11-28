mod common;
mod memory;

use memory::{T0, ok, send};
use openworks::Openworks;
use openworks_core::{Fail, Role, Row, Tx, Value};
use openworks_db::{
    AUDIT, EVENT, clients, lost_reasons, members, project_members, project_roles, projects, sites, stages,
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

impl Store {
    fn reaches(&self, project: &Row, person: &Value) -> bool {
        *person == Value::Null
            || self
                .staffed
                .iter()
                .any(|row| row[..2] == project[..2] && row[2] == *person)
    }

    fn change(&mut self, p: &[Value], set: impl FnOnce(&mut Row)) -> Vec<Row> {
        self.projects
            .iter_mut()
            .find(|row| row[..2] == p[..2])
            .map_or_else(Vec::new, |row| {
                set(row);
                vec![vec![row[1].clone()]]
            })
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
            projects::SAVE => self.change(p, |row| row[2..8].clone_from_slice(&p[2..8])),
            projects::MOVE => self.change(p, |row| {
                row[8] = p[2].clone();
                row[11..15].clone_from_slice(&p[3..7]);
            }),
            projects::LINK => self.change(p, |row| row[9..11].clone_from_slice(&p[2..4])),
            projects::ARCHIVE => self.change(p, |row| {
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
    let got = ok(send(openworks, "/projects/get", credential, id));
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
    assert_eq!(host.committed.borrow().audits, 9);
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
    let events = &host.committed.borrow().events;
    assert_eq!(events.last(), Some(&text("projects.save")));
}

#[test]
fn a_stage_move_follows_the_rules_of_its_kinds() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    set_up(&openworks);
    let id = project(&openworks, "Hall");
    let moving = |body: &str| status(&openworks, "/projects/move-stage", "owner", &format!("{id}\n{body}"));

    assert_eq!(moving("quoted").0, 200);
    assert_eq!(
        moving("done"),
        (
            400,
            "invalid: a project at a open stage does not move to a done stage".to_owned()
        )
    );
    host.clock.set(1);
    assert_eq!(moving("won").0, 200);
    assert_eq!(field(&openworks, "owner", &id, "won_at"), (T0 + 1).to_string());
    assert_eq!(moving("lost\nlate").0, 400);
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
    assert_eq!(
        host.committed.borrow().events.last(),
        Some(&text("projects.move-stage"))
    );
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
    host.clock.set(5);
    ok(send(&openworks, "/projects/archive", "owner", &id));

    assert_eq!(field(&openworks, "owner", &id, "archived_at"), T0.to_string());
    assert_eq!(send(&openworks, "/projects/archive", "other", &id).status, 404);
    assert_eq!(send(&openworks, "/projects/archive", "viewer", &id).status, 403);
    assert_eq!(host.committed.borrow().events.last(), Some(&text("projects.archive")));
}
