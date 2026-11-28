/// Lists a job's groups in order: id, job, name, order, and the ids of their tasks, comma-separated.
pub const GROUPS: &str = "select g.group_id, g.job_id, g.name, g.sort, coalesce((select string_agg(l.task_id, ',' order by l.task_id) from works.task_group_links l where l.org_id = g.org_id and l.group_id = g.group_id), '') from works.task_groups g where g.org_id = $1 and g.job_id = $2 order by g.sort, g.group_id";

/// Adds a job's group with its name and order.
pub const GROUP_ADD: &str =
    "insert into works.task_groups (org_id, group_id, job_id, name, sort) values ($1, $2, $3, $4, $5)";

/// Renames and reorders a job's group.
pub const GROUP_SAVE: &str = "update works.task_groups set name = $4, sort = $5 where org_id = $1 and group_id = $2 and job_id = $3 returning group_id";

/// Takes every task out of a group.
pub const UNLINK: &str = "delete from works.task_group_links where org_id = $1 and group_id = $2";

/// Puts a task in a group.
pub const LINK: &str =
    "insert into works.task_group_links (org_id, group_id, task_id) values ($1, $2, $3) on conflict do nothing";

/// Lists the company's templates by name: id, name, plan.
pub const TEMPLATES: &str =
    "select template_id, name, plan from works.plan_templates where org_id = $1 order by name, template_id";

/// Reads one template, with the columns `TEMPLATES` reads.
pub const TEMPLATE: &str =
    "select template_id, name, plan from works.plan_templates where org_id = $1 and template_id = $2";

/// Adds a template with its name and plan.
pub const TEMPLATE_ADD: &str =
    "insert into works.plan_templates (org_id, template_id, name, plan) values ($1, $2, $3, $4)";

/// Renames a template and changes its plan.
pub const TEMPLATE_SAVE: &str =
    "update works.plan_templates set name = $3, plan = $4 where org_id = $1 and template_id = $2 returning template_id";

/// Lists a job's versions by number: id, job, number, what the plan came from, what replaced it, the plan, when.
pub const VERSIONS: &str = "select version_id, job_id, number, came_from, replaced_by, plan, at from works.plan_versions where org_id = $1 and job_id = $2 order by number";

/// Reads one version, with the columns `VERSIONS` reads.
pub const VERSION: &str = "select version_id, job_id, number, came_from, replaced_by, plan, at from works.plan_versions where org_id = $1 and version_id = $2";

/// Keeps the plan a job had before it was replaced, numbered, with what it came from and what replaced it.
pub const KEEP: &str = "insert into works.plan_versions (org_id, version_id, job_id, number, came_from, replaced_by, plan, at) values ($1, $2, $3, $4, $5, $6, $7, $8)";
