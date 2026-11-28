mod memory;

use memory::{ok, send};
use openworks::{Offer, Openworks, Tool};
use openworks_core::{Fail, Role, Row, Tx, Value, Word};
use openworks_db::{AUDIT, EVENT, clients, members};

#[derive(Clone, Default)]
struct Store {
    clients: Vec<(String, String)>,
    audit: Vec<Value>,
    events: Vec<Value>,
}

fn text(value: &str) -> Value {
    Value::Text(value.to_owned())
}

impl Tx for Store {
    fn run(&mut self, sql: &'static str, p: &[Value]) -> Result<Vec<Row>, Fail> {
        Ok(match sql {
            members::FIND => Role::ALL
                .iter()
                .filter(|role| p[1] == text(role.name()))
                .map(|role| vec![text(role.name())])
                .collect(),
            clients::LIST => self
                .clients
                .iter()
                .map(|(id, name)| vec![text(id), text(name), Value::Null, Value::Null])
                .collect(),
            clients::ADD => {
                let (Value::Text(id), Value::Text(name)) = (&p[1], &p[2]) else {
                    panic!("a client is an id and a name");
                };
                self.clients.push((id.clone(), name.clone()));
                Vec::new()
            }
            AUDIT => {
                self.audit.push(p[3].clone());
                Vec::new()
            }
            EVENT => {
                self.events.push(p[2].clone());
                Vec::new()
            }
            _ => Vec::new(),
        })
    }
}

type TestHost = memory::TestHost<Store>;

const INITIALIZE: &str =
    r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"capabilities":{"elicitation":{}}}}"#;

fn served(host: &TestHost, token: &str, lines: &[&str]) -> Vec<String> {
    let openworks = Openworks::new(host).build().unwrap();
    let mut output = Vec::new();
    openworks
        .serve_mcp(token, lines.join("\n").as_bytes(), &mut output)
        .unwrap();
    String::from_utf8(output).unwrap().lines().map(str::to_owned).collect()
}

fn call(id: u32, tool: &str, body: &str) -> String {
    format!(
        r#"{{"jsonrpc":"2.0","id":{id},"method":"tools/call","params":{{"name":"{tool}","arguments":{{"body":"{body}"}}}}}}"#
    )
}

fn answered(id: u32, error: bool, body: &str) -> String {
    format!(
        r#"{{"jsonrpc":"2.0","id":{id},"result":{{"content":[{{"type":"text","text":"{body}"}}],"isError":{error}}}}}"#
    )
}

const ASK: &str = r#"{"jsonrpc":"2.0","id":"confirm-1","method":"elicitation/create","params":{"message":"Confirm /clients/save (write) with:\u000a\u000aAcme\u000a","requestedSchema":{"type":"object","properties":{}}}}"#;

fn reply(action: &str) -> String {
    format!(r#"{{"jsonrpc":"2.0","id":"confirm-1","result":{{"action":"{action}"}}}}"#)
}

#[test]
fn a_read_is_a_tool_a_write_a_proposal_and_a_delete_members_or_settings_never_one() {
    let host = TestHost::default();
    let tools = Openworks::new(&host).build().unwrap().tools();
    let offered = |path: &str| tools.iter().find(|tool| tool.path == path).map(|tool| tool.offer);

    assert_eq!(
        tools.iter().find(|tool| tool.path == "/job-orders/save"),
        Some(&Tool {
            name: "job-orders_save".to_owned(),
            path: "/job-orders/save".to_owned(),
            needs: Word::Write,
            offer: Offer::Proposal,
        })
    );
    for read in [
        "/clients/list",
        "/projects/changes",
        "/jobs/board",
        "/approvals/pending",
        "/files/list",
    ] {
        assert_eq!(offered(read), Some(Offer::Read), "{read}");
    }
    for write in ["/clients/save", "/jobs/open", "/tasks/archive", "/approvals/decide"] {
        assert_eq!(offered(write), Some(Offer::Proposal), "{write}");
    }
    for by_hand in [
        "/projects/archive",
        "/jobs/cancel",
        "/files/attach",
        "/orgs/members",
        "/orgs/set-member",
        "/orgs/rename",
        "/projects/stage-save",
        "/approvals/reset",
        "/exports/create",
    ] {
        assert_eq!(offered(by_hand), None, "{by_hand}");
    }
}

#[test]
fn the_server_lists_the_tools_with_the_word_each_needs() {
    let host = TestHost::default();

    let lines = served(
        &host,
        "manager",
        &[
            INITIALIZE,
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            r#"{"jsonrpc":"2.0","id":"l","method":"tools/list"}"#,
        ],
    );

    assert_eq!(lines.len(), 2);
    assert!(
        lines[0].starts_with(r#"{"jsonrpc":"2.0","id":0,"result":{"protocolVersion":"2025-06-18""#),
        "{}",
        lines[0]
    );
    assert!(
        lines[1].starts_with(r#"{"jsonrpc":"2.0","id":"l","result":{"tools":["#),
        "{}",
        lines[1]
    );
    assert!(lines[1].contains(r#"{"name":"clients_list","description":"Reads /clients/list; needs read","inputSchema":{"type":"object","properties":{"body":{"type":"string"}}},"annotations":{"readOnlyHint":true}}"#));
    assert!(lines[1].contains(
        r#""name":"clients_save","description":"Proposes, for the person to confirm, /clients/save; needs write""#
    ));
    assert!(!lines[1].contains("projects_archive"));
}

#[test]
fn a_read_tool_answers_as_the_token_signs_in() {
    let host = TestHost::default();
    host.store().clients.push(("c1".to_owned(), "Acme".to_owned()));

    let lines = served(&host, "viewer", &[INITIALIZE, &call(1, "clients_list", "")]);

    assert_eq!(
        lines[1],
        answered(
            1,
            false,
            r#"[{\"id\":\"c1\",\"name\":\"Acme\",\"host_ref\":null,\"deactivated_at\":null,\"ext\":{}}]"#
        )
    );
    assert_eq!(host.store().audit, Vec::<Value>::new());
}

#[test]
fn a_write_runs_once_the_person_confirms_it_with_one_audit_row_and_one_event() {
    let host = TestHost::default();

    let lines = served(
        &host,
        "manager",
        &[INITIALIZE, &call(1, "clients_save", "\\nAcme\\n"), &reply("accept")],
    );

    assert_eq!(lines[1..], [ASK.to_owned(), answered(1, false, "id2")]);
    let openworks = Openworks::new(&host).build().unwrap();
    assert_eq!(
        ok(send(&openworks, "/clients/list", "viewer", "")),
        r#"[{"id":"id2","name":"Acme","host_ref":null,"deactivated_at":null,"ext":{}}]"#
    );
    let store = host.store();
    assert_eq!(
        (store.audit.as_slice(), store.events.as_slice()),
        (&[text("clients.save")][..], &[text("clients.save")][..])
    );
}

#[test]
fn a_declined_proposal_writes_nothing() {
    let host = TestHost::default();

    let lines = served(
        &host,
        "manager",
        &[INITIALIZE, &call(1, "clients_save", "\\nAcme\\n"), &reply("decline")],
    );

    assert_eq!(
        lines[1..],
        [
            ASK.to_owned(),
            answered(1, true, "the person declined: nothing was written")
        ]
    );
    assert_eq!(host.store().clients, Vec::<(String, String)>::new());
    assert_eq!(host.store().audit, Vec::<Value>::new());
}

#[test]
fn a_client_that_cannot_ask_the_person_writes_nothing() {
    let host = TestHost::default();
    let initialize = r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"capabilities":{}}}"#;

    let lines = served(&host, "manager", &[initialize, &call(1, "clients_save", "\\nAcme\\n")]);

    assert_eq!(
        lines[1],
        answered(
            1,
            true,
            "this client cannot ask the person to confirm: nothing was written"
        )
    );
    assert_eq!(host.store().clients, Vec::<(String, String)>::new());
}

#[test]
fn a_confirmed_write_still_needs_the_word() {
    let host = TestHost::default();

    let lines = served(
        &host,
        "viewer",
        &[INITIALIZE, &call(1, "clients_save", "\\nAcme\\n"), &reply("accept")],
    );

    assert_eq!(lines[2], answered(1, true, "needs write"));
    assert_eq!(host.store().clients, Vec::<(String, String)>::new());
}

#[test]
fn a_token_no_member_holds_is_refused_and_a_by_hand_operation_is_no_tool() {
    let host = TestHost::default();

    let lines = served(
        &host,
        "stranger",
        &[
            INITIALIZE,
            &call(1, "clients_list", ""),
            &call(2, "projects_archive", "p1"),
            "{nope",
        ],
    );

    assert_eq!(lines[1], answered(1, true, "not a member"));
    assert_eq!(
        lines[2],
        r#"{"jsonrpc":"2.0","id":2,"error":{"code":-32602,"message":"no tool projects_archive"}}"#
    );
    assert_eq!(
        lines[3],
        r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"not JSON"}}"#
    );
}
