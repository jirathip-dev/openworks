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

/// Lists a project's tasks, job by job in order, with the columns `LIST` reads.
pub const BY_PROJECT: &str = "select t.task_id, t.job_id, t.code, t.name, t.color, t.start_offset, t.days, t.planned_hours, t.weight, t.sort, t.remark, t.archived_at from works.tasks t join works.jobs j on j.org_id = t.org_id and j.job_id = t.job_id where t.org_id = $1 and j.project_id = $2 order by j.job_id, t.sort, t.task_id";

/// Adds the task a host knows by a reference, on a job, unless it is there; returns its id either way.
pub const IMPORT: &str = "with added as (insert into works.tasks (org_id, task_id, job_id, code, name, start_offset, days, planned_hours, sort, host_ref) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) on conflict (org_id, host_ref) do nothing returning task_id) select task_id from added union all select task_id from works.tasks where org_id = $1 and host_ref = $10";
