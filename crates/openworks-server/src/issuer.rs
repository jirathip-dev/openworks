use std::cell::RefCell;

use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{DecodingKey, Validation, decode, decode_header};
use serde_json::Value as Json;

/// An OIDC issuer the server trusts to sign people in, by its discovery document and keys.
pub struct Issuer {
    url: String,
    client: String,
    jwks: String,
    keys: RefCell<JwkSet>,
}

/// The person a checked ID token names: the issuer's subject and their email.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Signed {
    pub subject: String,
    pub email: String,
}

impl Issuer {
    /// Reads the issuer's discovery document and its keys; ID tokens must name `client` as their audience.
    pub fn discover(url: &str, client: &str) -> Result<Self, String> {
        let found = fetch(&format!(
            "{}/.well-known/openid-configuration",
            url.trim_end_matches('/')
        ))?;
        let named = found.get("issuer").and_then(Json::as_str).unwrap_or_default();
        if named.trim_end_matches('/') != url.trim_end_matches('/') {
            return Err(format!("the issuer at {url} names itself {named}"));
        }
        let jwks = found
            .get("jwks_uri")
            .and_then(Json::as_str)
            .ok_or("the discovery document has no jwks_uri")?
            .to_owned();
        let keys = keys(&jwks)?;
        Ok(Self {
            url: named.to_owned(),
            client: client.to_owned(),
            jwks,
            keys: RefCell::new(keys),
        })
    }

    /// The issuer's URL, as it names itself.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// The client ID tokens are issued to.
    pub fn client(&self) -> &str {
        &self.client
    }

    /// Checks an ID token's signature, issuer, audience and expiry at `now` (seconds); answers who it signs in.
    pub fn check(&self, id_token: &str, now: i64) -> Result<Signed, String> {
        let header = decode_header(id_token).map_err(|error| error.to_string())?;
        let kid = header.kid.ok_or("the token names no key")?;
        if self.keys.borrow().find(&kid).is_none() {
            *self.keys.borrow_mut() = keys(&self.jwks)?;
        }
        let keys = self.keys.borrow();
        let jwk = keys.find(&kid).ok_or("the issuer has no such key")?;
        let key = DecodingKey::from_jwk(jwk).map_err(|error| error.to_string())?;
        let mut validation = Validation::new(header.alg);
        validation.set_issuer(&[&self.url]);
        validation.set_audience(&[&self.client]);
        validation.validate_exp = false;
        let claims = decode::<Json>(id_token, &key, &validation)
            .map_err(|error| error.to_string())?
            .claims;
        if claims.get("exp").and_then(Json::as_i64).is_none_or(|exp| exp <= now) {
            return Err("the token has expired".to_owned());
        }
        if claims.get("email_verified").and_then(Json::as_bool) == Some(false) {
            return Err("the email is not verified".to_owned());
        }
        let text = |name: &str| claims.get(name).and_then(Json::as_str).map(str::to_owned);
        Ok(Signed {
            subject: text("sub").ok_or("the token has no subject")?,
            email: text("email").ok_or("the token has no email")?,
        })
    }
}

fn keys(url: &str) -> Result<JwkSet, String> {
    serde_json::from_value(fetch(url)?).map_err(|error| error.to_string())
}

fn fetch(url: &str) -> Result<Json, String> {
    let body = ureq::get(url)
        .call()
        .and_then(|mut response| response.body_mut().read_to_string())
        .map_err(|error| format!("{url}: {error}"))?;
    serde_json::from_str(&body).map_err(|error| format!("{url}: {error}"))
}
