use openworks_core::{Declaration, Fail, Role, Tx, Value, Who};
use openworks_db::members::{HOLDING, LIST as LIST_SQL, REMOVE as REMOVE_SQL, SET as SET_SQL};

/// Lists the org's members, one `email role` a line.
pub const LIST: Declaration = openworks_core::read!("members", needs: Read, queries: [LIST_SQL], list);

/// Adds a member by email or changes their role, from a body of `email role`.
pub const SET: Declaration = openworks_core::write!("set-member", needs: Members, queries: [HOLDING, SET_SQL], set);

/// Removes the member with the body's email.
pub const REMOVE: Declaration =
    openworks_core::write!("remove-member", needs: Members, queries: [HOLDING, REMOVE_SQL], remove);

fn list(who: &Who, tx: &mut dyn Tx, _: &str) -> Result<String, Fail> {
    let lines = tx
        .run(LIST_SQL, &[org(who)])?
        .iter()
        .map(|row| match row.as_slice() {
            [Value::Text(email), Value::Text(role)] => Ok(format!("{email} {role}")),
            _ => Err(Fail::Store("a member row is not an email and a role".to_owned())),
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(lines.join("\n"))
}

fn set(who: &Who, tx: &mut dyn Tx, body: &str) -> Result<String, Fail> {
    let [email, role] = body.split_whitespace().collect::<Vec<_>>()[..] else {
        return Err(Fail::Invalid("a member needs an email and a role".to_owned()));
    };
    let email = address(email)?;
    let role = Role::named(role).ok_or_else(|| Fail::Invalid(format!("no role {role}")))?;
    keep_an_owner(who, tx, &email, role)?;
    tx.run(
        SET_SQL,
        &[
            org(who),
            Value::Text(email.clone()),
            Value::Text(role.name().to_owned()),
        ],
    )?;
    Ok(format!("{email} {}", role.name()))
}

fn remove(who: &Who, tx: &mut dyn Tx, body: &str) -> Result<String, Fail> {
    let email = address(body)?;
    keep_an_owner(who, tx, &email, Role::Viewer)?;
    tx.run(REMOVE_SQL, &[org(who), Value::Text(email.clone())])?;
    Ok(email)
}

fn keep_an_owner(who: &Who, tx: &mut dyn Tx, email: &str, becomes: Role) -> Result<(), Fail> {
    let owners = tx.run(HOLDING, &[org(who), Value::Text(Role::Owner.name().to_owned())])?;
    let another = owners
        .iter()
        .any(|row| row.as_slice() != [Value::Text(email.to_owned())]);
    if becomes == Role::Owner || another {
        Ok(())
    } else {
        Err(Fail::Invalid("a company always has an Owner".to_owned()))
    }
}

/// An email as members are keyed by it: trimmed and lowercased.
pub fn address(text: &str) -> Result<String, Fail> {
    let email = text.trim().to_lowercase();
    if email.contains('@') && !email.contains(char::is_whitespace) {
        Ok(email)
    } else {
        Err(Fail::Invalid(format!("not an email: {email}")))
    }
}

fn org(who: &Who) -> Value {
    Value::Text(who.org.0.clone())
}
