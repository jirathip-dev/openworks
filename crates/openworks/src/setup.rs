use openworks_core::{Call, Declaration, Fail, Record, StageKind, Tx, Value, Who};
use openworks_db::reports::{KIND_SAVE, KINDS};
use openworks_db::{lost_reasons, project_roles, stages};

use crate::clients::{lines, named, org};
use crate::projects::{record, records};
use crate::tasks::count;

/// Lists the company's stages in order, as records of kind `stages`.
pub const STAGES: Declaration =
    openworks_core::list!("stages", needs: Read, record: "stages", queries: [stages::LIST], list_stages);

/// Saves a stage from a body of `key`, `name`, `kind` (open, won, done or lost) and `sort` lines; answers its key.
pub const STAGE_SAVE: Declaration =
    openworks_core::write!("stage-save", needs: Settings, queries: [stages::SAVE], save_stage);

/// Lists the company's lost reasons, as records of kind `lost_reasons`.
pub const LOST_REASONS: Declaration = openworks_core::list!(
    "lost-reasons",
    needs: Read,
    record: "lost_reasons",
    queries: [lost_reasons::LIST],
    list_lost_reasons,
);

/// Saves a lost reason from a body of `key`, `name` and `needs_note` (yes or no) lines; answers its key.
pub const LOST_REASON_SAVE: Declaration =
    openworks_core::write!("lost-reason-save", needs: Settings, queries: [lost_reasons::SAVE], save_lost_reason);

/// Lists the company's project roles, as records of kind `project_roles`.
pub const ROLES: Declaration = openworks_core::list!(
    "roles",
    needs: Read,
    record: "project_roles",
    queries: [project_roles::LIST],
    list_roles,
);

/// Saves a project role from a body of `key` and `name` lines; answers its key.
pub const ROLE_SAVE: Declaration =
    openworks_core::write!("role-save", needs: Settings, queries: [project_roles::SAVE], save_role);

/// Lists the company's hour kinds in order, as records of kind `hour_kinds`.
pub const HOUR_KINDS: Declaration =
    openworks_core::list!("hour-kinds", needs: Read, record: "hour_kinds", queries: [KINDS], list_hour_kinds);

/// Saves an hour kind from a body of `key`, `name` and `sort` lines; answers its key.
pub const HOUR_KIND_SAVE: Declaration =
    openworks_core::write!("hour-kind-save", needs: Settings, queries: [KIND_SAVE], save_hour_kind);

fn list_stages(who: &Who, tx: &mut dyn Tx, _: &str) -> Result<Vec<Record>, Fail> {
    let rows = tx.run(stages::LIST, &[org(who)])?;
    rows.into_iter()
        .map(|row| record(row, &["name", "kind", "sort"]))
        .collect()
}

fn save_stage(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let [key, name, kind, sort] = lines(call.body)?;
    let kind = StageKind::named(kind).ok_or_else(|| Fail::Invalid(format!("no stage kind {kind}")))?;
    let sort = if sort.is_empty() { Ok(0) } else { sort.parse() };
    let sort = sort.map_err(|_| Fail::Invalid("a sort is a whole number".to_owned()))?;
    let params = [
        org(call.who),
        keyed(key)?,
        named(name)?,
        text(kind.name()),
        Value::Int(sort),
    ];
    tx.run(stages::SAVE, &params)?;
    Ok(key.to_owned())
}

fn list_lost_reasons(who: &Who, tx: &mut dyn Tx, _: &str) -> Result<Vec<Record>, Fail> {
    let rows = tx.run(lost_reasons::LIST, &[org(who)])?;
    rows.into_iter()
        .map(|row| record(row, &["name", "needs_note"]))
        .collect()
}

fn save_lost_reason(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let [key, name, needs_note] = lines(call.body)?;
    let needs_note = match needs_note {
        "yes" => 1,
        "no" | "" => 0,
        _ => return Err(Fail::Invalid("needs_note is yes or no".to_owned())),
    };
    let params = [org(call.who), keyed(key)?, named(name)?, Value::Int(needs_note)];
    tx.run(lost_reasons::SAVE, &params)?;
    Ok(key.to_owned())
}

fn list_roles(who: &Who, tx: &mut dyn Tx, _: &str) -> Result<Vec<Record>, Fail> {
    let rows = tx.run(project_roles::LIST, &[org(who)])?;
    rows.into_iter().map(|row| record(row, &["name"])).collect()
}

fn save_role(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let [key, name] = lines(call.body)?;
    tx.run(project_roles::SAVE, &[org(call.who), keyed(key)?, named(name)?])?;
    Ok(key.to_owned())
}

fn list_hour_kinds(who: &Who, tx: &mut dyn Tx, _: &str) -> Result<Vec<Record>, Fail> {
    records(tx.run(KINDS, &[org(who)])?, &["name", "sort"])
}

fn save_hour_kind(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let [key, name, sort] = lines(call.body)?;
    let sort = if sort.is_empty() { 0 } else { count("sort", sort)? };
    tx.run(KIND_SAVE, &[org(call.who), keyed(key)?, named(name)?, Value::Int(sort)])?;
    Ok(key.to_owned())
}

fn keyed(key: &str) -> Result<Value, Fail> {
    if key.is_empty() || key.contains(char::is_whitespace) {
        Err(Fail::Invalid("a key is one word".to_owned()))
    } else {
        Ok(text(key))
    }
}

fn text(value: &str) -> Value {
    Value::Text(value.to_owned())
}
