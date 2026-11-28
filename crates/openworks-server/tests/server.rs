mod issuer_double;

use std::env;
use std::path::PathBuf;

use issuer_double::{CLIENT, claims, signed, start};
use openworks::Host;
use openworks_core::{Id, Value};
use openworks_server::http::{Answer, Asked};
use openworks_server::{Gate, Issuer, Own, Server, Store};
use serde_json::{Value as Json, json};

const AUDITS: &str = "select count(*) from works.audit where org_id = $1";
const EVENTS: &str = "select count(*) from works.events where org_id = $1";

fn ask(server: &Server, method: &str, path: &str, token: &str, body: &str) -> Answer {
    server.answer(&Asked {
        method: method.to_owned(),
        path: path.to_owned(),
        headers: vec![
            ("Authorization".to_owned(), format!("Bearer {token}")),
            ("Idempotency-Key".to_owned(), format!("{path} {body}")),
        ],
        body: body.as_bytes().to_vec(),
    })
}

fn json_of(answer: &Answer) -> Json {
    serde_json::from_slice(&answer.body).unwrap()
}

fn said(answer: &Answer) -> String {
    String::from_utf8(answer.body.clone()).unwrap()
}

fn trail(store: &Store, org: &Id) -> (Value, Value) {
    let mut counts = (Value::Null, Value::Null);
    (&store)
        .transact(&mut |tx| {
            let org = [Value::Text(org.0.clone())];
            counts = (
                tx.run(AUDITS, &org)?.remove(0).remove(0),
                tx.run(EVENTS, &org)?.remove(0).remove(0),
            );
            Ok(())
        })
        .unwrap();
    counts
}

fn more(before: &(Value, Value), by: i64) -> (Value, Value) {
    match before {
        (Value::Int(audits), Value::Int(events)) => (Value::Int(audits + by), Value::Int(events + by)),
        _ => panic!("counts are numbers"),
    }
}

fn sign_in(server: &Server, store: &Store, org: &Id, issuer_url: &str) -> String {
    let now = (&store).now() / 1000;
    let stranger = json!({"org": org.0, "id_token": signed(&claims(issuer_url, "eve@o.test", now + 60))});
    assert_eq!(
        ask(server, "POST", "/auth/sign-in", "", &stranger.to_string()).status,
        403
    );

    let before = trail(store, org);
    let owner = json!({"org": org.0, "id_token": signed(&claims(issuer_url, "Owner@o.test", now + 60))});
    let signed_in = ask(server, "POST", "/auth/sign-in", "", &owner.to_string());
    assert_eq!(signed_in.status, 200, "{}", said(&signed_in));
    let person = json_of(&signed_in)["token"].as_str().unwrap().to_owned();
    assert_eq!(trail(store, org), more(&before, 1));

    assert_eq!(ask(server, "POST", "/orgs/list", "nobody", "").status, 401);
    let orgs = ask(server, "POST", "/orgs/list", &person, "");
    assert_eq!(orgs.status, 200, "{}", said(&orgs));
    assert!(said(&orgs).contains("Test works"));
    person
}

fn mint_assistant(server: &Server, person: &str) -> String {
    let minted = ask(server, "POST", "/auth/tokens", person, "");
    let assistant = json_of(&minted)["token"].as_str().unwrap().to_owned();
    assert_eq!(ask(server, "POST", "/auth/tokens", &assistant, "").status, 403);
    assert_eq!(ask(server, "POST", "/orgs/rename", &assistant, "Taken").status, 403);
    assert_eq!(ask(server, "POST", "/orgs/list", &assistant, "").status, 200);
    assistant
}

fn every_own_route_holds_its_gate(server: &Server, assistant: &str) {
    for own in Own::ALL {
        let (method, path) = own.route();
        let asked = if path.ends_with('/') {
            format!("{path}p1/confirm")
        } else {
            path.to_owned()
        };
        let anonymous = ask(server, method, &asked, "nobody", "").status;
        match own.gate() {
            Gate::Anyone => assert_ne!(anonymous, 401, "{own:?}"),
            Gate::Signed => assert_eq!(anonymous, 401, "{own:?}"),
            Gate::Person => {
                let by_assistant = ask(server, method, &asked, assistant, "").status;
                assert_eq!((anonymous, by_assistant), (401, 403), "{own:?}");
            }
        }
    }
}

fn propose_and_settle(server: &Server, store: &Store, org: &Id, person: &str, assistant: &str) {
    let before = trail(store, org);
    let proposed = ask(server, "POST", "/clients/save", assistant, "\nProposed client");
    assert_eq!(proposed.status, 202, "{}", said(&proposed));
    let proposal = json_of(&proposed)["proposal"].as_str().unwrap().to_owned();
    assert_eq!(trail(store, org), more(&before, 1));
    assert!(!said(&ask(server, "POST", "/clients/list", person, "")).contains("Proposed client"));

    assert_eq!(ask(server, "GET", "/inbox", assistant, "").status, 403);
    let inbox = json_of(&ask(server, "GET", "/inbox", person, ""));
    assert!(
        inbox
            .as_array()
            .unwrap()
            .iter()
            .any(|open| open["id"] == proposal.as_str())
    );

    let before = trail(store, org);
    let confirm = format!("/inbox/{proposal}/confirm");
    let confirmed = ask(server, "POST", &confirm, person, "");
    assert_eq!(confirmed.status, 200, "{}", said(&confirmed));
    assert_eq!(trail(store, org), more(&before, 1));
    assert!(said(&ask(server, "POST", "/clients/list", person, "")).contains("Proposed client"));
    assert_eq!(ask(server, "POST", &confirm, person, "").status, 404);

    let declined = json_of(&ask(server, "POST", "/clients/save", assistant, "\nDeclined client"));
    let decline = format!("/inbox/{}/decline", declined["proposal"].as_str().unwrap());
    assert_eq!(ask(server, "POST", &decline, person, "").status, 200);
    assert_eq!(ask(server, "POST", &decline, person, "").status, 404);
}

#[test]
fn a_person_signs_in_and_confirms_what_their_assistant_proposed() {
    let Ok(url) = env::var("OPENWORKS_TEST_DATABASE_URL") else {
        return;
    };
    let files = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("server-files");
    let store = Store::connect(&url, files).unwrap();
    store.migrate().unwrap();
    store.migrate().unwrap();
    let issuer_url = start();
    let issuer = Issuer::discover(&issuer_url, CLIENT).unwrap();
    let server = Server::new(&store, issuer, PathBuf::from("no-bundle")).unwrap();
    let org = server.init("Test works", "owner@o.test").unwrap();
    assert_eq!(server.init("Again", "someone@o.test").unwrap(), org);

    let person = sign_in(&server, &store, &org, &issuer_url);
    let assistant = mint_assistant(&server, &person);
    every_own_route_holds_its_gate(&server, &assistant);
    propose_and_settle(&server, &store, &org, &person, &assistant);

    assert_eq!(ask(&server, "POST", "/auth/sign-out", &assistant, "").status, 200);
    assert_eq!(ask(&server, "POST", "/orgs/list", &assistant, "").status, 401);
}
