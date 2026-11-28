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
    (
        "0006_clients_and_sites",
        include_str!("../migrations/0006_clients_and_sites.sql"),
    ),
    ("0007_projects", include_str!("../migrations/0007_projects.sql")),
    ("0008_jobs", include_str!("../migrations/0008_jobs.sql")),
    ("0009_job_orders", include_str!("../migrations/0009_job_orders.sql")),
    ("0010_tasks", include_str!("../migrations/0010_tasks.sql")),
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

/// The queries on clients.
pub mod clients {
    /// Lists every client: id, name, host reference and when it was deactivated.
    pub const LIST: &str = "select client_id, name, host_ref, deactivated_at from works.clients where org_id = $1 order by name, client_id";

    /// Reads one client: id, name, host reference and when it was deactivated.
    pub const GET: &str =
        "select client_id, name, host_ref, deactivated_at from works.clients where org_id = $1 and client_id = $2";

    /// Adds a client.
    pub const ADD: &str = "insert into works.clients (org_id, client_id, name, host_ref) values ($1, $2, $3, $4)";

    /// Changes a client's name and host reference.
    pub const SAVE: &str =
        "update works.clients set name = $3, host_ref = $4 where org_id = $1 and client_id = $2 returning client_id";

    /// Deactivates a client, keeping the first time it was.
    pub const DEACTIVATE: &str = "update works.clients set deactivated_at = coalesce(deactivated_at, $3) where org_id = $1 and client_id = $2 returning client_id";

    /// Adds the client the host knows by a reference, or renames it.
    pub const SYNC: &str = "insert into works.clients (org_id, client_id, name, host_ref) values ($1, $2, $3, $4) on conflict (org_id, host_ref) do update set name = excluded.name returning client_id";
}

/// The queries on sites.
pub mod sites {
    /// Lists a client's sites: id, client id, name, address, map link and when it was deactivated.
    pub const LIST: &str = "select site_id, client_id, name, address, map_url, deactivated_at from works.sites where org_id = $1 and client_id = $2 order by name, site_id";

    /// Reads one site: id, client id, name, address, map link and when it was deactivated.
    pub const GET: &str = "select site_id, client_id, name, address, map_url, deactivated_at from works.sites where org_id = $1 and site_id = $2";

    /// Adds a site under a client.
    pub const ADD: &str =
        "insert into works.sites (org_id, site_id, client_id, name, address, map_url) values ($1, $2, $3, $4, $5, $6)";

    /// Changes the name, address and map link of a site under a client.
    pub const SAVE: &str = "update works.sites set name = $4, address = $5, map_url = $6 where org_id = $1 and site_id = $2 and client_id = $3 returning site_id";

    /// Deactivates a site, keeping the first time it was.
    pub const DEACTIVATE: &str = "update works.sites set deactivated_at = coalesce(deactivated_at, $3) where org_id = $1 and site_id = $2 returning site_id";
}

/// The queries on the company's stages.
pub mod stages {
    /// Lists every stage: key, name, kind and sort.
    pub const LIST: &str =
        "select stage_key, name, kind, sort from works.stages where org_id = $1 order by sort, stage_key";

    /// Reads one stage: key, name, kind and sort.
    pub const GET: &str = "select stage_key, name, kind, sort from works.stages where org_id = $1 and stage_key = $2";

    /// Adds a stage by key, or changes its name, kind and sort.
    pub const SAVE: &str = "insert into works.stages (org_id, stage_key, name, kind, sort) values ($1, $2, $3, $4, $5) on conflict (org_id, stage_key) do update set name = excluded.name, kind = excluded.kind, sort = excluded.sort";
}

/// The queries on the company's lost reasons.
pub mod lost_reasons {
    /// Lists every lost reason: key, name and whether it needs a note.
    pub const LIST: &str =
        "select reason_key, name, needs_note from works.lost_reasons where org_id = $1 order by reason_key";

    /// Reads one lost reason: key, name and whether it needs a note.
    pub const GET: &str =
        "select reason_key, name, needs_note from works.lost_reasons where org_id = $1 and reason_key = $2";

    /// Adds a lost reason by key, or changes its name and whether it needs a note.
    pub const SAVE: &str = "insert into works.lost_reasons (org_id, reason_key, name, needs_note) values ($1, $2, $3, $4) on conflict (org_id, reason_key) do update set name = excluded.name, needs_note = excluded.needs_note";
}

/// The queries on the company's project roles.
pub mod project_roles {
    /// Lists every project role: key and name.
    pub const LIST: &str = "select role_key, name from works.project_roles where org_id = $1 order by role_key";

    /// Reads one project role: key and name.
    pub const GET: &str = "select role_key, name from works.project_roles where org_id = $1 and role_key = $2";

    /// Adds a project role by key, or renames it.
    pub const SAVE: &str = "insert into works.project_roles (org_id, role_key, name) values ($1, $2, $3) on conflict (org_id, role_key) do update set name = excluded.name";
}

/// The queries on projects; a person bound in place of null narrows a read to the projects they are a member of.
pub mod projects {
    /// Lists the projects: id, name, client, site, scope, tags, zone, stage, parent, link kind, won, lost, lost reason, lost note, archived.
    pub const LIST: &str = "select p.project_id, p.name, p.client_id, p.site_id, p.scope, p.tags, p.zone, p.stage_key, p.parent_id, p.link_kind, p.won_at, p.lost_at, p.lost_reason, p.lost_note, p.archived_at from works.projects p where p.org_id = $1 and ($2::text is null or exists (select 1 from works.project_members m where m.org_id = p.org_id and m.project_id = p.project_id and m.person_id = $2)) order by p.name, p.project_id";

    /// Reads one project, with the columns `LIST` reads.
    pub const GET: &str = "select p.project_id, p.name, p.client_id, p.site_id, p.scope, p.tags, p.zone, p.stage_key, p.parent_id, p.link_kind, p.won_at, p.lost_at, p.lost_reason, p.lost_note, p.archived_at from works.projects p where p.org_id = $1 and p.project_id = $2 and ($3::text is null or exists (select 1 from works.project_members m where m.org_id = p.org_id and m.project_id = p.project_id and m.person_id = $3))";

    /// Adds a project at a stage.
    pub const ADD: &str = "insert into works.projects (org_id, project_id, name, client_id, site_id, scope, tags, zone, stage_key) values ($1, $2, $3, $4, $5, $6, $7, $8, $9)";

    /// Changes a project's name, client, site, scope, tags and zone.
    pub const SAVE: &str = "update works.projects set name = $3, client_id = $4, site_id = $5, scope = $6, tags = $7, zone = $8 where org_id = $1 and project_id = $2 returning project_id";

    /// Moves a project to a stage with its won and lost marks.
    pub const MOVE: &str = "update works.projects set stage_key = $3, won_at = $4, lost_at = $5, lost_reason = $6, lost_note = $7 where org_id = $1 and project_id = $2 returning project_id";

    /// Links a project to a parent with a kind, or unlinks it with both null.
    pub const LINK: &str = "update works.projects set parent_id = $3, link_kind = $4 where org_id = $1 and project_id = $2 returning project_id";

    /// Archives a project, keeping the first time it was.
    pub const ARCHIVE: &str = "update works.projects set archived_at = coalesce(archived_at, $3) where org_id = $1 and project_id = $2 returning project_id";
}

/// The queries on the people of a project.
pub mod project_members {
    /// Lists a project's people: person and role.
    pub const LIST: &str = "select person_id, role_key from works.project_members where org_id = $1 and project_id = $2 order by person_id, role_key";

    /// Removes every person from a project.
    pub const CLEAR: &str = "delete from works.project_members where org_id = $1 and project_id = $2";

    /// Adds a person to a project in a role.
    pub const ADD: &str = "insert into works.project_members (org_id, project_id, person_id, role_key) values ($1, $2, $3, $4) on conflict do nothing";
}

/// The queries on jobs; a person bound in place of null narrows a read to the jobs of the projects they are a member of.
pub mod jobs {
    /// Lists a project's jobs: id, project, number, status, at site, base date, work days, planned start and end, actual start and end, remark, finished.
    pub const LIST: &str = "select j.job_id, j.project_id, j.number, j.status, j.at_site, j.base_date, j.work_days, j.planned_start, j.planned_end, j.actual_start, j.actual_end, j.remark, j.finished_at from works.jobs j where j.org_id = $1 and j.project_id = $2 order by j.job_id";

    /// Reads one job, with the columns `LIST` reads.
    pub const GET: &str = "select j.job_id, j.project_id, j.number, j.status, j.at_site, j.base_date, j.work_days, j.planned_start, j.planned_end, j.actual_start, j.actual_end, j.remark, j.finished_at from works.jobs j where j.org_id = $1 and j.job_id = $2 and ($3::text is null or exists (select 1 from works.project_members m where m.org_id = j.org_id and m.project_id = j.project_id and m.person_id = $3))";

    /// Adds a project's job, proposed.
    pub const ADD: &str = "insert into works.jobs (org_id, job_id, project_id, status) values ($1, $2, $3, 'proposed')";

    /// Changes whether a job is at the site, its calendar, its actual start and its remark.
    pub const SAVE: &str = "update works.jobs set at_site = $3, base_date = $4, work_days = $5, actual_start = $6, remark = $7 where org_id = $1 and job_id = $2 returning job_id";

    /// Sets a job's planned window.
    pub const PLAN: &str =
        "update works.jobs set planned_start = $3, planned_end = $4 where org_id = $1 and job_id = $2 returning job_id";

    /// Moves a job to a status with its number, actual end and finished mark.
    pub const MOVE: &str = "update works.jobs set status = $3, number = $4, actual_end = $5, finished_at = $6 where org_id = $1 and job_id = $2 returning job_id";
}

/// The queries on job orders, one row per revision of a job's instruction, and their lines.
pub mod job_orders {
    /// Lists a job's revisions, oldest first: id, job, revision, status, issued by, issued at.
    pub const LIST: &str = "select job_order_id, job_id, revision, status, issued_by, issued_at from works.job_orders where org_id = $1 and job_id = $2 order by revision";

    /// Reads one revision, with the columns `LIST` reads.
    pub const GET: &str = "select job_order_id, job_id, revision, status, issued_by, issued_at from works.job_orders where org_id = $1 and job_order_id = $2";

    /// Reads the id of a job's draft revision.
    pub const DRAFT: &str =
        "select job_order_id from works.job_orders where org_id = $1 and job_id = $2 and status = 'draft'";

    /// Adds a job's next revision as a draft.
    pub const ADD: &str = "insert into works.job_orders (org_id, job_order_id, job_id, revision, status) select $1, $2, $3, coalesce(max(revision), 0) + 1, 'draft' from works.job_orders where org_id = $1 and job_id = $3";

    /// Lists a revision's lines in order.
    pub const LINES: &str =
        "select body from works.job_order_lines where org_id = $1 and job_order_id = $2 order by line";

    /// Removes every line of a revision.
    pub const CLEAR: &str = "delete from works.job_order_lines where org_id = $1 and job_order_id = $2";

    /// Adds a line to a revision at its place.
    pub const ADD_LINE: &str =
        "insert into works.job_order_lines (org_id, job_order_id, line, body) values ($1, $2, $3, $4)";

    /// Marks a job's issued revision superseded.
    pub const SUPERSEDE: &str =
        "update works.job_orders set status = 'superseded' where org_id = $1 and job_id = $2 and status = 'issued'";

    /// Issues a draft revision, by whom and when; returns its id when it was a draft.
    pub const ISSUE: &str = "update works.job_orders set status = 'issued', issued_by = $3, issued_at = $4 where org_id = $1 and job_order_id = $2 and status = 'draft' returning job_order_id";
}

/// The queries on tasks; a person bound in place of null narrows a read to the tasks of the projects they are a member of.
pub mod tasks {
    /// Lists a job's tasks in order: id, job, code, name, colour, start offset, days, planned hours, weight, order, remark, archived.
    pub const LIST: &str = "select t.task_id, t.job_id, t.code, t.name, t.color, t.start_offset, t.days, t.planned_hours, t.weight, t.sort, t.remark, t.archived_at from works.tasks t where t.org_id = $1 and t.job_id = $2 order by t.sort, t.task_id";

    /// Reads one task, with the columns `LIST` reads.
    pub const GET: &str = "select t.task_id, t.job_id, t.code, t.name, t.color, t.start_offset, t.days, t.planned_hours, t.weight, t.sort, t.remark, t.archived_at from works.tasks t join works.jobs j on j.org_id = t.org_id and j.job_id = t.job_id where t.org_id = $1 and t.task_id = $2 and ($3::text is null or exists (select 1 from works.project_members m where m.org_id = j.org_id and m.project_id = j.project_id and m.person_id = $3))";

    /// Adds a job's task with its code, name, colour, start offset, days, planned hours, weight, order and remark.
    pub const ADD: &str = "insert into works.tasks (org_id, task_id, job_id, code, name, color, start_offset, days, planned_hours, weight, sort, remark) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)";

    /// Changes a job's task: code, name, colour, start offset, days, planned hours, weight, order and remark.
    pub const SAVE: &str = "update works.tasks set code = $4, name = $5, color = $6, start_offset = $7, days = $8, planned_hours = $9, weight = $10, sort = $11, remark = $12 where org_id = $1 and task_id = $2 and job_id = $3 returning task_id";

    /// Archives a task, keeping the first time it was.
    pub const ARCHIVE: &str = "update works.tasks set archived_at = coalesce(archived_at, $3) where org_id = $1 and task_id = $2 returning task_id";

    /// Restores an archived task.
    pub const RESTORE: &str =
        "update works.tasks set archived_at = null where org_id = $1 and task_id = $2 returning task_id";
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
