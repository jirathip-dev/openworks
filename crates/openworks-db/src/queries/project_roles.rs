/// Lists every project role: key and name.
pub const LIST: &str = "select role_key, name from works.project_roles where org_id = $1 order by role_key";

/// Reads one project role: key and name.
pub const GET: &str = "select role_key, name from works.project_roles where org_id = $1 and role_key = $2";

/// Adds a project role by key, or renames it.
pub const SAVE: &str = "insert into works.project_roles (org_id, role_key, name) values ($1, $2, $3) on conflict (org_id, role_key) do update set name = excluded.name";
