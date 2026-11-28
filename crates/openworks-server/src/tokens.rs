use openworks::{Caller, Via};
use openworks_core::{Fail, Id, Tx, Value};
use sha2::{Digest, Sha256};

use crate::store::{hex, random, via};

const FIND: &str = "select org_id, person_id, email, via, created_at from server.tokens where token_hash = $1";

const LIFETIME_MS: i64 = 30 * 24 * 60 * 60 * 1000;

const SAVE: &str =
    "insert into server.tokens (token_hash, org_id, person_id, email, via, created_at) values ($1, $2, $3, $4, $5, $6)";

const DROP: &str = "delete from server.tokens where token_hash = $1 and org_id = $2 returning org_id";

pub(crate) fn find(tx: &mut dyn Tx, token: &str, now: i64) -> Result<Option<Caller>, Fail> {
    let rows = tx.run(FIND, &[Value::Text(hashed(token))])?;
    Ok(match rows.into_iter().next().as_deref() {
        Some(
            [
                Value::Text(org),
                Value::Text(person),
                Value::Text(email),
                Value::Text(via),
                Value::Int(created_at),
            ],
        ) if now - created_at < LIFETIME_MS => Some(Caller {
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

#[cfg(test)]
mod tests {
    use openworks::{Caller, Via};
    use openworks_core::{Fail, Id, Row, Tx, Value};

    use super::{FIND, LIFETIME_MS, SAVE, find, issue};

    #[derive(Default)]
    struct Memory(Vec<Row>);

    impl Tx for Memory {
        fn run(&mut self, sql: &'static str, params: &[Value]) -> Result<Vec<Row>, Fail> {
            Ok(match sql {
                SAVE => {
                    self.0.push(params.to_vec());
                    Vec::new()
                }
                FIND => self
                    .0
                    .iter()
                    .filter_map(|row| row.split_first())
                    .filter(|(hash, _)| params.first() == Some(*hash))
                    .map(|(_, rest)| rest.to_vec())
                    .collect(),
                _ => Vec::new(),
            })
        }
    }

    #[test]
    fn a_token_stops_working_once_its_lifetime_has_passed() -> Result<(), Fail> {
        let caller = Caller {
            person: Id("p".to_owned()),
            email: "owner@o.test".to_owned(),
            org: Id("o".to_owned()),
            via: Via::Person,
        };
        let mut tx = Memory::default();
        let at = 1_000;
        let token = issue(&mut tx, &caller, at)?;
        let found = find(&mut tx, &token, at + LIFETIME_MS - 1)?;
        assert_eq!(found.map(|found| found.person), Some(caller.person));
        assert!(find(&mut tx, &token, at + LIFETIME_MS)?.is_none());
        Ok(())
    }
}
