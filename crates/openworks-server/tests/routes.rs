use openworks::{Caller, Via};
use openworks_core::Id;
use openworks_server::{Gate, Own};

fn caller(via: Via) -> Caller {
    Caller {
        person: Id("p1".to_owned()),
        email: "owner@o.test".to_owned(),
        org: Id("o1".to_owned()),
        via,
    }
}

fn status(gate: Gate, caller: Option<&Caller>) -> u16 {
    gate.refuses(caller).map_or(200, |refused| refused.status)
}

#[test]
fn every_route_the_server_answers_itself_is_declared_with_who_may_call_it() {
    let declared = Own::ALL.map(|own| (own.route(), own.gate()));
    assert_eq!(
        declared,
        [
            (("GET", "/auth/config"), Gate::Anyone),
            (("POST", "/auth/sign-in"), Gate::Anyone),
            (("POST", "/auth/tokens"), Gate::Person),
            (("POST", "/auth/sign-out"), Gate::Signed),
            (("GET", "/openapi.json"), Gate::Anyone),
            (("GET", "/inbox"), Gate::Person),
            (("POST", "/inbox/"), Gate::Person),
        ]
    );
    for own in Own::ALL {
        let (method, path) = own.route();
        let asked = if path.ends_with('/') {
            format!("{path}p1/confirm")
        } else {
            path.to_owned()
        };
        assert_eq!(Own::find(method, &asked), Some(own), "{method} {asked}");
    }
    assert_eq!(Own::find("POST", "/inbox"), None);
    assert_eq!(Own::find("GET", "/inbox/p1/confirm"), None);
    assert_eq!(Own::find("POST", "/orgs/list"), None);
}

#[test]
fn a_gate_lets_through_only_whom_it_declares() {
    let person = caller(Via::Person);
    let assistant = caller(Via::Assistant);
    for own in Own::ALL {
        let gate = own.gate();
        let expected = match gate {
            Gate::Anyone => [200, 200, 200],
            Gate::Signed => [401, 200, 200],
            Gate::Person => [401, 200, 403],
        };
        assert_eq!(
            [
                status(gate, None),
                status(gate, Some(&person)),
                status(gate, Some(&assistant))
            ],
            expected,
            "{own:?}"
        );
    }
}
