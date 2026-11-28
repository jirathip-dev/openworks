/// Lists a project's people: person and role.
pub const LIST: &str = "select person_id, role_key from works.project_members where org_id = $1 and project_id = $2 order by person_id, role_key";

/// Removes every person from a project.
pub const CLEAR: &str = "delete from works.project_members where org_id = $1 and project_id = $2";

/// Adds a person to a project in a role.
pub const ADD: &str = "insert into works.project_members (org_id, project_id, person_id, role_key) values ($1, $2, $3, $4) on conflict do nothing";
