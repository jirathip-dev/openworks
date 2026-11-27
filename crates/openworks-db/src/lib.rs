use openworks_core::{Fail, Tx};

/// Every migration, by name, in the order it runs.
pub const MIGRATIONS: &[(&str, &str)] = &[
    ("0001_orgs", include_str!("../../../migrations/0001_orgs.sql")),
    ("0002_members", include_str!("../../../migrations/0002_members.sql")),
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
}

/// The queries on members.
pub mod members {
    /// Reads the role of the member tied to a person.
    pub const FIND: &str = "select role from works.members where org_id = $1 and person_id = $2";

    /// Ties the member added by an email to the person signing in, once.
    pub const TIE: &str =
        "update works.members set person_id = $2 where org_id = $1 and email = $3 and person_id is null returning role";

    /// Reads every member's email and role.
    pub const LIST: &str = "select email, role from works.members where org_id = $1 order by email";

    /// Reads the emails of the members who hold a role.
    pub const HOLDING: &str = "select email from works.members where org_id = $1 and role = $2";

    /// Adds a member by email, or changes their role.
    pub const SET: &str = "insert into works.members (org_id, email, role) values ($1, $2, $3) on conflict (org_id, email) do update set role = excluded.role";

    /// Removes a member by email.
    pub const REMOVE: &str = "delete from works.members where org_id = $1 and email = $2";
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
