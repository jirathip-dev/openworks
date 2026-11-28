use openworks::{Caller, Via};
use openworks_core::{Fail, Id, Tx, Value};
use sha2::{Digest, Sha256};

use crate::store::{hex, random, via};

const FIND: &str = "select org_id, person_id, email, via from server.tokens where token_hash = $1";

const SAVE: &str =
    "insert into server.tokens (token_hash, org_id, person_id, email, via, created_at) values ($1, $2, $3, $4, $5, $6)";

const DROP: &str = "delete from server.tokens where token_hash = $1 and org_id = $2 returning org_id";

pub(crate) fn find(tx: &mut dyn Tx, token: &str) -> Result<Option<Caller>, Fail> {
    let rows = tx.run(FIND, &[Value::Text(hashed(token))])?;
    Ok(match rows.into_iter().next().as_deref() {
        Some(
            [
                Value::Text(org),
                Value::Text(person),
                Value::Text(email),
                Value::Text(via),
            ],
        ) => Some(Caller {
            person: Id(person.clone()),
            email: email.clone(),
            org: Id(org.clone()),
            via: if via == "assistant" {
                Via::Assistant
            } else {
                Via::Person
            },
        }),
        _ => None,
    })
}

pub(crate) fn issue(tx: &mut dyn Tx, caller: &Caller, at: i64) -> Result<String, Fail> {
    let token = hex(&random::<32>()?);
    tx.run(
        SAVE,
        &[
            Value::Text(hashed(&token)),
            Value::Text(caller.org.0.clone()),
            Value::Text(caller.person.0.clone()),
            Value::Text(caller.email.clone()),
            Value::Text(via(caller).to_owned()),
            Value::Int(at),
        ],
    )?;
    Ok(token)
}

pub(crate) fn revoke(tx: &mut dyn Tx, caller: &Caller, token: &str) -> Result<(), Fail> {
    let rows = tx.run(DROP, &[Value::Text(hashed(token)), Value::Text(caller.org.0.clone())])?;
    if rows.is_empty() { Err(Fail::NotFound) } else { Ok(()) }
}

fn hashed(token: &str) -> String {
    hex(&Sha256::digest(token.as_bytes()))
}
