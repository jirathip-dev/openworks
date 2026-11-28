use openworks_core::{Change, Fail, Kind, Tx, Value};

pub(crate) fn create_key(kind: Kind, key: Option<&str>) -> Result<Option<&str>, Fail> {
    match (kind, key) {
        (Kind::Create, None | Some("")) => Err(Fail::Invalid("a create needs an Idempotency-Key".to_owned())),
        (Kind::Create, key) => Ok(key),
        _ => Ok(None),
    }
}

pub(crate) fn replay(tx: &mut dyn Tx, change: &Change, key: &str) -> Result<Option<String>, Fail> {
    let rows = tx.run(
        openworks_db::idempotency::FIND,
        &[
            Value::Text(change.org.0.clone()),
            Value::Text(change.operation.clone()),
            Value::Text(key.to_owned()),
        ],
    )?;
    Ok(match rows.first().and_then(|row| row.first()) {
        Some(Value::Text(answer)) => Some(answer.clone()),
        _ => None,
    })
}

pub(crate) fn keep(tx: &mut dyn Tx, change: &Change, key: &str, answer: &str) -> Result<(), Fail> {
    tx.run(
        openworks_db::idempotency::SAVE,
        &[
            Value::Text(change.org.0.clone()),
            Value::Text(change.operation.clone()),
            Value::Text(key.to_owned()),
            Value::Text(answer.to_owned()),
            Value::Int(change.at),
        ],
    )?;
    Ok(())
}
