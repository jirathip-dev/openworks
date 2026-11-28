mod common;

use common::{OWNER, TestHost, call, staffed};
use openworks::{Openworks, Response};
use openworks_core::{Fail, Id, Role, Value};
use openworks_db::members;

#[test]
fn a_member_is_added_by_email_and_tied_at_first_sign_in() {
    let host: TestHost = TestHost::default();
    let openworks = staffed(&host, None);

    let added = call(&openworks, "/orgs/set-member", OWNER, " New@O.test  manager");
    assert_eq!(added.body, "new@o.test manager");
    assert_eq!(
        host.committed
            .borrow()
            .members
            .last()
            .and_then(|member| member.person.clone()),
        None
    );

    assert_eq!(call(&openworks, "/orgs/rename", "new@o.test", "x").status, 403);
    assert_eq!(call(&openworks, "/orgs/members", "new@o.test", "").status, 200);
    assert_eq!(call(&openworks, "/orgs/members", "new@o.test", "").status, 200);

    let tied = host
        .committed
        .borrow()
        .members
        .last()
        .and_then(|member| member.person.clone());
    assert_eq!(tied.as_deref(), Some("person:new@o.test"));
    assert_eq!(host.count(members::TIE), 1);
    let joined = Value::Text("orgs.join".to_owned());
    let committed = host.committed.borrow();
    let audit = committed
        .statements
        .iter()
        .filter(|(sql, params)| *sql == openworks_db::AUDIT && params.get(3) == Some(&joined));
    let event = committed
        .statements
        .iter()
        .filter(|(sql, params)| *sql == openworks_db::EVENT && params.get(2) == Some(&joined));
    assert_eq!((audit.count(), event.count()), (1, 1));
}

#[test]
fn a_person_who_is_no_member_is_refused() {
    let host: TestHost = TestHost::default();
    let openworks = staffed(&host, None);

    let refused = call(&openworks, "/orgs/list", "stranger@o.test", "");

    assert_eq!(
        refused,
        Response {
            status: 403,
            body: "not a member".to_owned()
        }
    );
}

#[test]
fn a_company_always_has_an_owner() {
    let host: TestHost = TestHost::default();
    let openworks = staffed(&host, None);
    let org = || Id("o1".to_owned());
    let by = || Id("host".to_owned());

    assert_eq!(call(&openworks, "/orgs/remove-member", OWNER, OWNER).status, 400);
    assert_eq!(
        call(&openworks, "/orgs/set-member", OWNER, "owner@o.test viewer").status,
        400
    );
    let lone = openworks.set_member(by(), org(), OWNER, Role::Manager);
    assert_eq!(lone, Err(Fail::Invalid("a company always has an Owner".to_owned())));
    let owner = host
        .committed
        .borrow()
        .members
        .iter()
        .find(|member| member.email == OWNER)
        .map(|member| member.role.clone());
    assert_eq!(owner.as_deref(), Some("owner"));

    let empty: TestHost = TestHost::default();
    let fresh = Openworks::new(&empty).build().unwrap();
    assert!(fresh.set_member(by(), org(), "first@o.test", Role::Viewer).is_err());
    assert_eq!(empty.committed.borrow().members.len(), 0);

    assert_eq!(
        call(&openworks, "/orgs/set-member", OWNER, "manager@o.test owner").status,
        200
    );
    assert_eq!(
        call(&openworks, "/orgs/remove-member", "manager@o.test", OWNER).status,
        200
    );
    assert_eq!(
        call(&openworks, "/orgs/members", "manager@o.test", "").body,
        "manager@o.test owner\nmember@o.test member\nviewer@o.test viewer"
    );
}

#[test]
fn a_host_sets_members_through_the_api() {
    let host: TestHost = TestHost::default();
    let openworks = Openworks::new(&host).build().unwrap();

    let set = openworks.set_member(Id("host".to_owned()), Id("o1".to_owned()), "boss@o.test", Role::Owner);

    assert_eq!(set, Ok("boss@o.test owner".to_owned()));
    assert_eq!(call(&openworks, "/orgs/rename", "boss@o.test", "Renamed").status, 200);
    assert_eq!(host.count(openworks_db::AUDIT), 3);
    assert_eq!(host.count(openworks_db::EVENT), 3);
}
