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

/// Lists the jobs not cancelled whose planned window meets the days from `$2` to `$3`, by start, with the columns `LIST` reads.
pub const BOARD: &str = "select j.job_id, j.project_id, j.number, j.status, j.at_site, j.base_date, j.work_days, j.planned_start, j.planned_end, j.actual_start, j.actual_end, j.remark, j.finished_at from works.jobs j where j.org_id = $1 and j.status <> 'cancelled' and j.planned_start <= $3 and j.planned_end >= $2 and ($4::text is null or exists (select 1 from works.project_members m where m.org_id = j.org_id and m.project_id = j.project_id and m.person_id = $4)) order by j.planned_start, j.job_id";

/// Adds the job a host knows by a reference, with its status, number and planned window, unless it is there; returns its id either way.
pub const IMPORT: &str = "with added as (insert into works.jobs (org_id, job_id, project_id, status, number, planned_start, planned_end, host_ref) values ($1, $2, $3, $4, $5, $6, $7, $8) on conflict (org_id, host_ref) do nothing returning job_id) select job_id from added union all select job_id from works.jobs where org_id = $1 and host_ref = $8";
