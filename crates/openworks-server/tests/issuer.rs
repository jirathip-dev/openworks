mod issuer_double;

use issuer_double::{CLIENT, claims, signed, signed_by, start};
use openworks_server::{Issuer, Signed};

const NOW: i64 = 1_700_000_000;

#[test]
fn a_token_the_issuer_signed_for_this_client_signs_its_person_in() {
    let url = start();
    let issuer = Issuer::discover(&url, CLIENT).unwrap();

    let token = signed(&claims(&url, "ana@o.test", NOW + 60));

    assert_eq!(
        issuer.check(&token, NOW),
        Ok(Signed {
            subject: "sub-ana@o.test".to_owned(),
            email: "ana@o.test".to_owned(),
        })
    );
    assert_eq!(issuer.url(), url);
}

#[test]
fn an_expired_token_is_refused_by_the_clock_it_is_given() {
    let url = start();
    let issuer = Issuer::discover(&url, CLIENT).unwrap();
    let token = signed(&claims(&url, "ana@o.test", NOW + 60));

    assert_eq!(issuer.check(&token, NOW + 60), Err("the token has expired".to_owned()));
}

#[test]
fn a_token_for_another_client_or_issuer_is_refused() {
    let url = start();
    let issuer = Issuer::discover(&url, CLIENT).unwrap();
    let mut other_client = claims(&url, "ana@o.test", NOW + 60);
    other_client["aud"] = "someone-else".into();
    let other_issuer = claims("http://elsewhere.test", "ana@o.test", NOW + 60);

    assert!(issuer.check(&signed(&other_client), NOW).is_err());
    assert!(issuer.check(&signed(&other_issuer), NOW).is_err());
}

#[test]
fn a_tampered_token_or_an_unknown_key_is_refused() {
    let url = start();
    let issuer = Issuer::discover(&url, CLIENT).unwrap();
    let token = signed(&claims(&url, "ana@o.test", NOW + 60));
    let (head, signature) = token.rsplit_once('.').unwrap();
    let forged = signed(&claims(&url, "eve@o.test", NOW + 60));
    let forged_body = forged.split('.').nth(1).unwrap();
    let swapped = format!("{}.{forged_body}.{signature}", head.split('.').next().unwrap());

    assert!(issuer.check(&swapped, NOW).is_err());
    assert_eq!(
        issuer.check(&signed_by("k9", &claims(&url, "ana@o.test", NOW + 60)), NOW),
        Err("the issuer has no such key".to_owned())
    );
}

#[test]
fn an_unverified_email_is_refused() {
    let url = start();
    let issuer = Issuer::discover(&url, CLIENT).unwrap();
    let mut unverified = claims(&url, "ana@o.test", NOW + 60);
    unverified["email_verified"] = false.into();

    assert_eq!(
        issuer.check(&signed(&unverified), NOW),
        Err("the email is not verified".to_owned())
    );
}
