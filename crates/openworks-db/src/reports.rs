/// Lists the company's hour kinds in order: key, name and sort.
pub const KINDS: &str = "select kind_key, name, sort from works.hour_kinds where org_id = $1 order by sort, kind_key";

/// Reads one hour kind's key.
pub const KIND: &str = "select kind_key from works.hour_kinds where org_id = $1 and kind_key = $2";

/// Adds an hour kind by key, or changes its name and sort.
pub const KIND_SAVE: &str = "insert into works.hour_kinds (org_id, kind_key, name, sort) values ($1, $2, $3, $4) on conflict (org_id, kind_key) do update set name = excluded.name, sort = excluded.sort";

/// Lists a job's reports by day: id, job, day, number, status and version.
pub const LIST: &str = "select report_id, job_id, day, number, status, version from works.daily_reports where org_id = $1 and job_id = $2 order by day";

/// Reads one report, with the columns `LIST` reads.
pub const GET: &str = "select report_id, job_id, day, number, status, version from works.daily_reports where org_id = $1 and report_id = $2";

/// Reads the id of a job's report on a day.
pub const FIND: &str = "select report_id from works.daily_reports where org_id = $1 and job_id = $2 and day = $3";

/// Adds a job's report on a day with its number, as a draft at version 1.
pub const ADD: &str = "insert into works.daily_reports (org_id, report_id, job_id, day, number, status, version) values ($1, $2, $3, $4, $5, 'draft', 1)";

/// Moves a report to its next version.
pub const EDIT: &str =
    "update works.daily_reports set version = version + 1 where org_id = $1 and report_id = $2 returning report_id";

/// Lists a report's hours: worker, task, kind and hundredths of an hour.
pub const HOURS: &str = "select worker_id, task_id, kind_key, hours from works.report_hours where org_id = $1 and report_id = $2 order by worker_id, task_id, kind_key";

/// Removes every hour of a report.
pub const CLEAR_HOURS: &str = "delete from works.report_hours where org_id = $1 and report_id = $2";

/// Adds a report's hours of a worker on a task by kind.
pub const ADD_HOURS: &str = "insert into works.report_hours (org_id, report_id, worker_id, task_id, kind_key, hours) values ($1, $2, $3, $4, $5, $6)";

/// Lists a report's declared progress: task and percent.
pub const PROGRESS: &str =
    "select task_id, percent from works.report_progress where org_id = $1 and report_id = $2 order by task_id";

/// Removes every declared progress of a report.
pub const CLEAR_PROGRESS: &str = "delete from works.report_progress where org_id = $1 and report_id = $2";

/// Adds a report's declared progress on a task.
pub const ADD_PROGRESS: &str =
    "insert into works.report_progress (org_id, report_id, task_id, percent) values ($1, $2, $3, $4)";

/// Sums a job's hours by report and kind, by day, leaving voided reports out: report, day, kind and hundredths.
pub const BY_DAY: &str = "select r.report_id, r.day, h.kind_key, cast(sum(h.hours) as bigint) from works.daily_reports r join works.report_hours h on h.org_id = r.org_id and h.report_id = r.report_id where r.org_id = $1 and r.job_id = $2 and r.status <> 'voided' group by r.report_id, r.day, h.kind_key order by r.day, h.kind_key";

/// Sums the hundredths of an hour reported on a task.
pub const REPORTED: &str =
    "select cast(coalesce(sum(hours), 0) as bigint) from works.report_hours where org_id = $1 and task_id = $2";
