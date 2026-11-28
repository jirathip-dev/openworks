use std::net::TcpListener;
use std::thread;

use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use openworks_server::http::{self, Answer};
use serde_json::{Value, json};

pub const CLIENT: &str = "openworks-test";

const KEY: &[u8] = include_bytes!("../fixtures/test-issuer.pem");
const JWKS: &str = include_str!("../fixtures/test-issuer-jwks.json");

pub fn start() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let issuer = url.clone();
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let asked = http::read(&stream).unwrap();
            let answer = match asked.path.as_str() {
                "/.well-known/openid-configuration" => Answer::json(
                    200,
                    json!({"issuer": issuer, "jwks_uri": format!("{issuer}/jwks")}).to_string(),
                ),
                "/jwks" => Answer::json(200, JWKS.to_owned()),
                _ => Answer::text(404, "no"),
            };
            http::write(&stream, &answer).unwrap();
        }
    });
    url
}

pub fn signed(claims: &Value) -> String {
    signed_by("k1", claims)
}

pub fn signed_by(kid: &str, claims: &Value) -> String {
    let header = Header {
        kid: Some(kid.to_owned()),
        ..Header::new(Algorithm::RS256)
    };
    encode(&header, claims, &EncodingKey::from_rsa_pem(KEY).unwrap()).unwrap()
}

pub fn claims(issuer: &str, email: &str, exp: i64) -> Value {
    json!({"iss": issuer, "aud": CLIENT, "sub": format!("sub-{email}"), "email": email, "exp": exp})
}
