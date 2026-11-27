use std::cell::{Cell, RefCell};

use openworks::{Caller, Files, Host, Openworks, Request, Response};
use openworks_core::{Approval, Fail, Id, Row, Tx, Value, Word};
use openworks_pack_example::{CHANGES, LONGEST, pack};

#[derive(Default)]
struct PackHost {
    page: usize,
    statements: RefCell<Vec<&'static str>>,
    transactions: Cell<usize>,
}

struct Store<'a> {
    page: usize,
    statements: &'a mut Vec<&'static str>,
}

impl Tx for Store<'_> {
    fn run(&mut self, sql: &'static str, _: &[Value]) -> Result<Vec<Row>, Fail> {
        self.statements.push(sql);
        Ok(match sql {
            openworks_db::orgs::LIST => (1..=self.page)
                .map(|n| vec![Value::Text(format!("o{n}")), Value::Text(format!("Org {n}"))])
                .collect(),
            openworks_db::members::FIND => vec![vec![Value::Text("owner".to_owned())]],
            CHANGES => vec![vec![Value::Text("o1".to_owned()), Value::Int(3)]],
            _ => Vec::new(),
        })
    }
}

struct NoFiles;

impl Files for NoFiles {
    fn put(&self, _: &str, _: &[u8]) -> Result<(), Fail> {
        Ok(())
    }
    fn get(&self, _: &str) -> Result<Option<Vec<u8>>, Fail> {
        Ok(None)
    }
}

impl Host for &PackHost {
    fn who(&self, credential: &str) -> Option<Caller> {
        (credential == "member").then(|| Caller {
            person: Id("p1".to_owned()),
            email: "p1@o.test".to_owned(),
            org: Id("o1".to_owned()),
        })
    }

    fn transact(&self, work: &mut dyn FnMut(&mut dyn Tx) -> Result<(), Fail>) -> Result<(), Fail> {
        self.transactions.set(self.transactions.get() + 1);
        let mut pending = self.statements.borrow().clone();
        work(&mut Store {
            page: self.page,
            statements: &mut pending,
        })?;
        *self.statements.borrow_mut() = pending;
        Ok(())
    }

    fn id(&self) -> Id {
        Id("id".to_owned())
    }

    fn now(&self) -> i64 {
        0
    }

    fn files(&self) -> &dyn Files {
        &NoFiles
    }

    fn name(&self, _: &str) -> Option<String> {
        None
    }
}

impl PackHost {
    fn count(&self, sql: &str) -> usize {
        self.statements.borrow().iter().filter(|run| **run == sql).count()
    }
}

fn call(openworks: &Openworks<&PackHost>, path: &str, body: &str) -> Response {
    openworks.handle(&Request {
        path: path.to_owned(),
        credential: "member".to_owned(),
        idempotency_key: None,
        body: body.to_owned(),
    })
}

#[test]
fn a_list_read_is_one_core_query_and_one_extension_call_whatever_the_page_size() {
    for page in [1, 2, 50] {
        let host = PackHost {
            page,
            ..PackHost::default()
        };
        let openworks = Openworks::new(&host).pack(pack()).build().unwrap();

        let listed = call(&openworks, "/orgs/list", "");

        assert_eq!(listed.status, 200, "{}", listed.body);
        assert_eq!(host.transactions.get(), 1);
        assert_eq!(host.count(openworks_db::orgs::LIST), 1);
        assert_eq!(host.count(CHANGES), 1);
        assert_eq!(listed.body.matches(r#""ext":{"example":"#).count(), page);
        assert!(
            listed
                .body
                .starts_with(r#"[{"id":"o1","name":"Org 1","ext":{"example":{"changes":3}}}"#),
            "{}",
            listed.body
        );
        if page > 1 {
            assert!(
                listed
                    .body
                    .contains(r#"{"id":"o2","name":"Org 2","ext":{"example":null}}"#)
            );
        }
    }
}

#[test]
fn the_openapi_document_types_the_extension() {
    let host = PackHost::default();
    let openworks = Openworks::new(&host).pack(pack()).build().unwrap();

    let document = openworks.openapi();

    let ext = r#""ext":{"type":"object","properties":{"example":{"type":["object","null"],"properties":{"changes":{"type":"integer"}}}}}"#;
    assert!(document.contains(ext), "{document}");
    assert!(
        document.contains(r#""/example/whoami":{"post":{"x-needs":"read","#),
        "{document}"
    );
}

#[test]
fn the_pack_brings_its_route_approval_kind_and_print() {
    let host = PackHost::default();
    let openworks = Openworks::new(&host).pack(pack()).build().unwrap();

    assert_eq!(
        call(&openworks, "/example/whoami", ""),
        Response {
            status: 200,
            body: "p1".to_owned()
        }
    );
    assert_eq!(
        openworks.approvals(),
        [Approval {
            key: "example.handover",
            needs: Word::Approve,
        }]
    );
    let prints: Vec<_> = openworks
        .prints()
        .iter()
        .map(|print| (print.key, print.record))
        .collect();
    assert_eq!(prints, [("example.summary", "orgs")]);
}

#[test]
fn the_pack_hook_refuses_an_oversized_change_and_nothing_is_kept() {
    let host = PackHost::default();
    let openworks = Openworks::new(&host).pack(pack()).build().unwrap();

    let refused = call(&openworks, "/orgs/rename", &"x".repeat(LONGEST + 1));
    assert_eq!(refused.status, 400);
    assert_eq!(host.statements.borrow().len(), 0);

    assert_eq!(call(&openworks, "/orgs/rename", "New name").status, 200);
    assert_eq!(host.count(openworks_db::EVENT), 1);
}
