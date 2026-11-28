use openworks_core::{Fail, Tx};

use crate::MIGRATIONS;

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

/// One table as an export writes it: its name, its columns and the query that reads an org's rows in order.
pub struct Table {
    pub name: &'static str,
    pub columns: &'static [&'static str],
    pub select: &'static str,
}

macro_rules! select {
    ($name:literal: $first:literal $(, $column:literal)*) => {
        concat!(
            "select ", $first $(, ", ", $column)*,
            " from works.", $name, " where org_id = $1 order by ", $first $(, ", ", $column)*
        )
    };
}

macro_rules! tables {
    ($($name:literal: [$($column:literal),+]),+ $(,)?) => {
        /// Every table, each with every column but `org_id`, in the order the migrations create them.
        pub const TABLES: &[Table] = &[$(Table {
            name: $name,
            columns: &[$($column),+],
            select: select!($name: $($column),+),
        }),+];

        /// The queries `TABLES` reads with.
        pub const SELECTS: &[&str] = &[$(select!($name: $($column),+)),+];
    };
}

tables! {
    "orgs": ["name", "created_at"],
    "audit": ["audit_id", "person_id", "operation", "at"],
    "events": ["event_id", "kind", "body", "at", "seq"],
    "idempotency": ["operation", "idempotency_key", "answer", "at"],
    "attachments": ["attachment_id", "tag", "name", "size", "at"],
    "series": ["series_key", "period", "last"],
    "members": ["email", "person_id", "role"],
    "approval_kinds": ["kind_key", "signature"],
    "approvals": [
        "approval_id", "kind_key", "subject", "state", "asked_by", "asked_at", "asked_note", "proposed",
        "proposed_by", "proposed_note", "decided_by", "decided_at", "decided_note", "signature"
    ],
    "clients": ["client_id", "name", "host_ref", "deactivated_at"],
    "sites": ["site_id", "client_id", "name", "address", "map_url", "deactivated_at", "host_ref"],
    "stages": ["stage_key", "name", "kind", "sort"],
    "lost_reasons": ["reason_key", "name", "needs_note"],
    "project_roles": ["role_key", "name"],
    "projects": [
        "project_id", "name", "client_id", "site_id", "scope", "tags", "zone", "stage_key", "parent_id",
        "link_kind", "won_at", "lost_at", "lost_reason", "lost_note", "archived_at", "host_ref"
    ],
    "project_members": ["project_id", "person_id", "role_key"],
    "jobs": [
        "job_id", "project_id", "number", "status", "at_site", "base_date", "work_days", "planned_start",
        "planned_end", "actual_start", "actual_end", "remark", "finished_at", "host_ref"
    ],
    "job_orders": ["job_order_id", "job_id", "revision", "status", "issued_by", "issued_at"],
    "job_order_lines": ["job_order_id", "line", "body"],
    "tasks": [
        "task_id", "job_id", "code", "name", "color", "start_offset", "days", "planned_hours", "weight", "sort",
        "remark", "archived_at", "host_ref"
    ],
    "teams": ["team_id", "name"],
    "workers": ["worker_id", "name", "title", "team_id", "company", "person_id", "active"],
    "equipment": ["equipment_id", "tag", "name", "kind", "place", "holder_job_id", "status"],
    "equipment_bookings": ["booking_id", "equipment_id", "job_id", "task_id", "starts_on", "ends_on", "status"],
    "equipment_needs": ["need_id", "job_id", "task_id", "kind", "starts_on", "ends_on", "booking_id"],
    "task_groups": ["group_id", "job_id", "name", "sort"],
    "task_group_links": ["group_id", "task_id"],
    "plan_templates": ["template_id", "name", "plan"],
    "plan_versions": ["version_id", "job_id", "number", "came_from", "replaced_by", "plan", "at"],
    "seats": ["seat_id", "job_id", "title", "worker_id", "number"],
    "seat_days": ["seat_id", "day"],
}
