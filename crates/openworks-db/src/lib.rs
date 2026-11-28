use openworks_core::{Fail, Tx};

/// Every migration, by name, in the order it runs.
pub const MIGRATIONS: &[(&str, &str)] = &[
    ("0001_orgs", include_str!("../migrations/0001_orgs.sql")),
    (
        "0002_events_seq_and_keys",
        include_str!("../migrations/0002_events_seq_and_keys.sql"),
    ),
    (
        "0003_attachments_and_series",
        include_str!("../migrations/0003_attachments_and_series.sql"),
    ),
    ("0004_members", include_str!("../migrations/0004_members.sql")),
    ("0005_approvals", include_str!("../migrations/0005_approvals.sql")),
];

/// Leaves the audit row of a write.
pub const AUDIT: &str =
    "insert into works.audit (audit_id, org_id, person_id, operation, at) values ($1, $2, $3, $4, $5)";

/// Leaves the event of a write.
pub const EVENT: &str = "insert into works.events (event_id, org_id, kind, body, at) values ($1, $2, $3, $4, $5)";

/// The queries on orgs.
pub mod orgs {
    /// Lists the orgs the caller reads: id and name.
    pub const LIST: &str = "select org_id, name from works.orgs where org_id = $1 order by org_id";

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

/// The queries on attachments.
pub mod attachments {
    /// Records a file the host stored, under its tag.
    pub const ADD: &str =
        "insert into works.attachments (org_id, attachment_id, tag, name, size, at) values ($1, $2, $3, $4, $5, $6)";

    /// Finds a file by its id.
    pub const FIND: &str = "select attachment_id from works.attachments where org_id = $1 and attachment_id = $2";

    /// Lists the files under a tag, oldest first.
    pub const LIST: &str = "select attachment_id, name, size from works.attachments where org_id = $1 and tag = $2 order by at, attachment_id";
}

/// The queries on number series.
pub mod series {
    /// Takes the next number of a series in a period, locking its counter row until the transaction ends.
    pub const NEXT: &str = "insert into works.series (org_id, series_key, period, last) values ($1, $2, $3, 1) on conflict (org_id, series_key, period) do update set last = works.series.last + 1 returning last";
}

/// The queries on approval kinds and approvals.
pub mod approvals {
    /// Reads whether a company's own kind asks for a signature.
    pub const KIND: &str = "select signature from works.approval_kinds where org_id = $1 and kind_key = $2";

    /// Adds a company's own kind, or changes whether it asks for a signature.
    pub const SAVE_KIND: &str = "insert into works.approval_kinds (org_id, kind_key, signature) values ($1, $2, $3) on conflict (org_id, kind_key) do update set signature = excluded.signature";

    /// Asks for an approval unless one of its kind on its subject is pending or approved; returns its id when asked.
    pub const REQUEST: &str = "insert into works.approvals (org_id, approval_id, kind_key, subject, state, asked_by, asked_at, asked_note) values ($1, $2, $3, $4, 'pending', $5, $6, $7) on conflict (org_id, kind_key, subject) where state in ('pending', 'approved') do nothing returning approval_id";

    /// Reads the kind of a pending approval.
    pub const OPEN: &str =
        "select kind_key from works.approvals where org_id = $1 and approval_id = $2 and state = 'pending'";

    /// Records an assistant's proposed decision on a pending approval.
    pub const PROPOSE: &str = "update works.approvals set proposed = $3, proposed_by = $4, proposed_note = $5 where org_id = $1 and approval_id = $2 and state = 'pending'";

    /// Decides a pending approval: approved or rejected, by whom, when, the note and the signature file.
    pub const DECIDE: &str = "update works.approvals set state = $3, decided_by = $4, decided_at = $5, decided_note = $6, signature = $7 where org_id = $1 and approval_id = $2 and state = 'pending'";

    /// Resets an approved approval to none; returns its id when it was approved.
    pub const RESET: &str = "update works.approvals set state = 'reset' where org_id = $1 and approval_id = $2 and state = 'approved' returning approval_id";

    /// Lists the pending approvals, oldest first, with any assistant's proposal.
    pub const PENDING: &str = "select approval_id, kind_key, subject, asked_by, asked_note, proposed, proposed_note from works.approvals where org_id = $1 and state = 'pending' order by asked_at, approval_id";

    /// Lists the latest decided approvals, newest first.
    pub const RECENT: &str = "select approval_id, kind_key, subject, state, decided_by, decided_note, signature from works.approvals where org_id = $1 and decided_at is not null order by decided_at desc, approval_id limit 50";
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
