mod common;

use std::cell::{Cell, RefCell};

use common::NoFiles;
use openworks::{Caller, Files, Host, Openworks, Request, Response, Via};
use openworks_core::{Fail, Id, Role, Row, Tx, Value};
use openworks_db::{AUDIT, EVENT, clients, members, sites};

const T0: i64 = 1_700_000_000_000;

#[derive(Clone, Default)]
struct Store {
    clients: Vec<Row>,
    sites: Vec<Row>,
    events: Vec<(Value, Value)>,
    audits: usize,
}

fn text(value: &str) -> Value {
    Value::Text(value.to_owned())
}

fn select(table: &[Row], hit: impl Fn(&Row) -> bool) -> Vec<Row> {
    table
        .iter()
        .filter(|row| hit(row))
        .map(|row| row[1..].to_vec())
        .collect()
}

fn change(table: &mut [Row], hit: impl Fn(&Row) -> bool, set: impl FnOnce(&mut Row)) -> Vec<Row> {
    table.iter_mut().find(|row| hit(row)).map_or_else(Vec::new, |row| {
        set(row);
        vec![vec![row[1].clone()]]
    })
}

fn deactivate(at: &Value) -> impl FnOnce(&mut Row) {
    move |row| {
        if let Some(last) = row.last_mut().filter(|last| **last == Value::Null) {
            *last = at.clone();
        }
    }
}

impl Tx for Store {
    fn run(&mut self, sql: &'static str, p: &[Value]) -> Result<Vec<Row>, Fail> {
        let keyed = |row: &Row| row[0] == p[0] && row[1] == p[1];
        let by_ref = |row: &Row| row[0] == p[0] && row[3] == p[3];
        Ok(match sql {
            members::FIND => vec![vec![text(match &p[1] {
                Value::Text(person) if person == "viewer" => Role::Viewer.name(),
                _ => Role::Owner.name(),
            })]],
            AUDIT => {
                self.audits += 1;
                Vec::new()
            }
            EVENT => {
                self.events.push((p[2].clone(), p[3].clone()));
                Vec::new()
            }
            clients::LIST => {
                let mut rows = select(&self.clients, |row| row[0] == p[0]);
                rows.sort_by_key(|row| format!("{:?}", row[1]));
                rows
            }
            clients::GET => select(&self.clients, keyed),
            clients::ADD => {
                if p[3] != Value::Null && self.clients.iter().any(by_ref) {
                    return Err(Fail::Store("host_ref taken".to_owned()));
                }
                self.clients.push([&p[..4], &[Value::Null]].concat());
                Vec::new()
            }
            clients::SYNC if !self.clients.iter().any(by_ref) => {
                self.clients.push([&p[..4], &[Value::Null]].concat());
                vec![vec![p[1].clone()]]
            }
            clients::SYNC => change(&mut self.clients, by_ref, |row| row[2] = p[2].clone()),
            clients::SAVE => change(&mut self.clients, keyed, |row| row[2..4].clone_from_slice(&p[2..4])),
            clients::DEACTIVATE => change(&mut self.clients, keyed, deactivate(&p[2])),
            sites::LIST => select(&self.sites, |row| row[0] == p[0] && row[2] == p[1]),
            sites::GET => select(&self.sites, keyed),
            sites::ADD => {
                self.sites.push([&p[..6], &[Value::Null]].concat());
                Vec::new()
            }
            sites::SAVE => change(
                &mut self.sites,
                |row| keyed(row) && row[2] == p[2],
                |row| row[3..6].clone_from_slice(&p[3..6]),
            ),
            sites::DEACTIVATE => change(&mut self.sites, keyed, deactivate(&p[2])),
            _ => Vec::new(),
        })
    }
}

#[derive(Default)]
struct TestHost {
    committed: RefCell<Store>,
    next_id: Cell<u32>,
    clock: Cell<i64>,
}

impl Host for &TestHost {
    fn who(&self, credential: &str) -> Option<Caller> {
        let org = if credential == "other" { "o2" } else { "o1" };
        Some(Caller {
            person: Id(credential.to_owned()),
            email: format!("{credential}@o.test"),
            org: Id(org.to_owned()),
            via: Via::Person,
        })
    }

    fn transact(&self, work: &mut dyn FnMut(&mut dyn Tx) -> Result<(), Fail>) -> Result<(), Fail> {
        let mut pending = self.committed.borrow().clone();
        work(&mut pending)?;
        *self.committed.borrow_mut() = pending;
        Ok(())
    }

    fn id(&self) -> Id {
        self.next_id.set(self.next_id.get() + 1);
        Id(format!("id{}", self.next_id.get()))
    }

    fn now(&self) -> i64 {
        T0 + self.clock.get()
    }

    fn files(&self) -> &dyn Files {
        &NoFiles
    }

    fn name(&self, _: &str) -> Option<String> {
        None
    }
}

fn send(openworks: &Openworks<&TestHost>, path: &str, credential: &str, body: &str) -> Response {
    openworks.handle(&Request {
        path: path.to_owned(),
        credential: credential.to_owned(),
        idempotency_key: Some(format!("{path} {body}")),
        body: body.to_owned(),
    })
}

fn ok(response: Response) -> String {
    assert_eq!(response.status, 200, "{}", response.body);
    response.body
}

#[test]
fn a_client_is_saved_listed_got_and_renamed() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();

    let id = ok(send(&openworks, "/clients/save", "owner", "\nAcme\nhost-7"));
    let beta = ok(send(&openworks, "/clients/save", "owner", "\nBeta"));

    assert_eq!(
        ok(send(&openworks, "/clients/list", "viewer", "")),
        format!(
            r#"[{{"id":"{id}","name":"Acme","host_ref":"host-7","deactivated_at":null,"ext":{{}}}},{{"id":"{beta}","name":"Beta","host_ref":null,"deactivated_at":null,"ext":{{}}}}]"#
        )
    );
    assert_eq!(
        ok(send(&openworks, "/clients/save", "owner", &format!("{id}\nAcme Two\n"))),
        id
    );
    assert_eq!(
        ok(send(&openworks, "/clients/get", "viewer", &id)),
        format!(r#"[{{"id":"{id}","name":"Acme Two","host_ref":null,"deactivated_at":null,"ext":{{}}}}]"#)
    );
    assert_eq!(send(&openworks, "/clients/get", "viewer", "nobody").status, 404);
    assert_eq!(send(&openworks, "/clients/save", "owner", "nobody\nX").status, 404);
    assert_eq!(send(&openworks, "/clients/save", "owner", "\n  \n").status, 400);
    assert_eq!(send(&openworks, "/clients/save", "viewer", "\nAcme").status, 403);
    assert_eq!(ok(send(&openworks, "/clients/list", "other", "")), "[]");
    assert_eq!(host.committed.borrow().audits, 3);
    assert_eq!(host.committed.borrow().events.len(), 3);
}

#[test]
fn a_client_is_deactivated_once_and_keeps_its_first_time() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    let id = ok(send(&openworks, "/clients/save", "owner", "\nAcme"));

    ok(send(&openworks, "/clients/deactivate", "owner", &id));
    host.clock.set(5);
    ok(send(&openworks, "/clients/deactivate", "owner", &id));

    assert!(ok(send(&openworks, "/clients/get", "owner", &id)).contains(&format!(r#""deactivated_at":{T0},"#)));
    assert_eq!(send(&openworks, "/clients/deactivate", "other", &id).status, 404);
    assert_eq!(
        host.committed.borrow().events.last().unwrap().0,
        text("clients.deactivate")
    );
}

#[test]
fn a_host_syncs_its_clients_by_reference_and_twice_does_what_once_did() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    let (by, org) = (Id("host".to_owned()), Id("o1".to_owned()));

    let first = openworks
        .sync_client(by.clone(), org.clone(), "cust-1", "Acme")
        .unwrap();
    let again = openworks
        .sync_client(by.clone(), org.clone(), "cust-1", "Acme Ltd")
        .unwrap();
    let routed = ok(send(&openworks, "/clients/sync", "owner", "cust-1\nAcme Group"));

    assert_eq!((again.as_str(), routed.as_str()), (first.as_str(), first.as_str()));
    assert_eq!(
        ok(send(&openworks, "/clients/list", "owner", "")),
        format!(r#"[{{"id":"{first}","name":"Acme Group","host_ref":"cust-1","deactivated_at":null,"ext":{{}}}}]"#)
    );
    assert_eq!(
        openworks.sync_client(by, org, "", "Nameless"),
        Err(Fail::Invalid("a synced client needs a host_ref".to_owned()))
    );
    let events = &host.committed.borrow().events;
    assert_eq!(events.len(), 3);
    assert_eq!(events[0], (text("clients.sync"), text("cust-1\nAcme")));
}

#[test]
fn a_site_lives_under_a_client_with_an_address_and_a_map_link() {
    let host = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();
    let client = ok(send(&openworks, "/clients/save", "owner", "\nAcme"));
    let map = "https://maps.example/?q=1";

    let site = ok(send(
        &openworks,
        "/sites/save",
        "owner",
        &format!("\n{client}\nYard\n1 Main Road\n{map}"),
    ));
    ok(send(
        &openworks,
        "/sites/save",
        "owner",
        &format!("{site}\n{client}\nYard\n2 Main Road\n{map}"),
    ));

    let expected = format!(
        r#"[{{"id":"{site}","client_id":"{client}","name":"Yard","address":"2 Main Road","map_url":"{map}","deactivated_at":null,"ext":{{}}}}]"#
    );
    assert_eq!(ok(send(&openworks, "/sites/list", "viewer", &client)), expected);
    assert_eq!(ok(send(&openworks, "/sites/get", "viewer", &site)), expected);
    assert_eq!(ok(send(&openworks, "/sites/list", "viewer", "nobody")), "[]");
    assert_eq!(send(&openworks, "/sites/save", "owner", "\nnobody\nYard").status, 404);
    assert_eq!(
        send(&openworks, "/sites/save", "owner", &format!("{site}\nnobody\nYard")).status,
        404
    );
    assert_eq!(
        send(
            &openworks,
            "/sites/save",
            "owner",
            &format!("\n{client}\nYard\n\nnot a link")
        )
        .status,
        400
    );
    ok(send(&openworks, "/sites/deactivate", "owner", &site));
    assert!(ok(send(&openworks, "/sites/get", "viewer", &site)).contains(&format!(r#""deactivated_at":{T0},"#)));
    assert_eq!(send(&openworks, "/sites/get", "other", &site).status, 404);
}
