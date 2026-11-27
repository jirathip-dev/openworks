use openworks_core::{Fail, Tx};

/// Every migration, by name, in the order it runs.
pub const MIGRATIONS: &[(&str, &str)] = &[
    ("0001_orgs", include_str!("../../../migrations/0001_orgs.sql")),
    (
        "0002_events_seq_and_keys",
        include_str!("../../../migrations/0002_events_seq_and_keys.sql"),
    ),
];

/// Leaves the audit row of a write.
pub const AUDIT: &str =
    "insert into works.audit (audit_id, org_id, person_id, operation, at) values ($1, $2, $3, $4, $5)";

/// Leaves the event of a write.
pub const EVENT: &str = "insert into works.events (event_id, org_id, kind, body, at) values ($1, $2, $3, $4, $5)";

/// The queries on orgs.
pub mod orgs {
    /// Reads the caller's org name.
    pub const SHOW: &str = "select name from works.orgs where org_id = $1";

    /// Renames the caller's org.
    pub const RENAME: &str = "update works.orgs set name = $2 where org_id = $1";

    /// Lists every org, for due work.
    pub const ALL: &str = "select org_id from works.orgs";
}

/// The queries on idempotency keys.
pub mod idempotency {
    /// Reads the first answer to a create under its key.
    pub const FIND: &str =
        "select answer from works.idempotency where org_id = $1 and operation = $2 and idempotency_key = $3";

    /// Keeps a create's answer under its key.
    pub const SAVE: &str =
        "insert into works.idempotency (org_id, operation, idempotency_key, answer, at) values ($1, $2, $3, $4, $5)";

    /// Forgets an org's keys kept before a time.
    pub const PRUNE: &str = "delete from works.idempotency where org_id = $1 and at < $2";
}

/// Runs every migration's statements in the transaction; each is idempotent.
pub fn migrate(tx: &mut dyn Tx) -> Result<(), Fail> {
    for (_, sql) in MIGRATIONS {
        for statement in sql.split(';').map(str::trim) {
            if !statement.is_empty() {
                tx.run(statement, &[])?;
            }
        }
    }
    Ok(())
}

/// The queries that touch a `works` table without naming `org_id`.
pub fn unscoped<'a>(queries: impl IntoIterator<Item = &'a str>) -> Vec<&'a str> {
    queries
        .into_iter()
        .filter(|sql| {
            let mut names = sql.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '.'));
            let touches_works = names.clone().any(|name| name.starts_with("works."));
            touches_works && !names.any(|name| name == "org_id" || name.ends_with(".org_id"))
        })
        .collect()
}
