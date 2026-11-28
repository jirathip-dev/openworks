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
pub const LINES: &str = "select body from works.job_order_lines where org_id = $1 and job_order_id = $2 order by line";

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
