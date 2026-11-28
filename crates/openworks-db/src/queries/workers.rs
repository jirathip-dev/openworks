/// Lists the org's workers by name: id, name, title, team, company (null for own staff), person and active.
pub const LIST: &str = "select worker_id, name, title, team_id, company, person_id, active from works.workers where org_id = $1 order by name, worker_id";

/// Reads one worker, with the columns `LIST` reads.
pub const GET: &str = "select worker_id, name, title, team_id, company, person_id, active from works.workers where org_id = $1 and worker_id = $2";

/// Reads the worker a person signs in as, with the columns `LIST` reads.
pub const ME: &str = "select worker_id, name, title, team_id, company, person_id, active from works.workers where org_id = $1 and person_id = $2";

/// Adds a worker with their name, title, team, company and person.
pub const ADD: &str = "insert into works.workers (org_id, worker_id, name, title, team_id, company, person_id) values ($1, $2, $3, $4, $5, $6, $7)";

/// Changes a worker's name, title, team, company and person.
pub const SAVE: &str = "update works.workers set name = $3, title = $4, team_id = $5, company = $6, person_id = $7 where org_id = $1 and worker_id = $2 returning worker_id";

/// Sets whether a worker is active.
pub const SET_ACTIVE: &str =
    "update works.workers set active = $3 where org_id = $1 and worker_id = $2 returning worker_id";
