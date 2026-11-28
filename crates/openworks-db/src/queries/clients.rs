/// Lists every client: id, name, host reference and when it was deactivated.
pub const LIST: &str =
    "select client_id, name, host_ref, deactivated_at from works.clients where org_id = $1 order by name, client_id";

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

/// Adds the client a host knows by a reference unless it is there; returns its id either way.
pub const IMPORT: &str = "with added as (insert into works.clients (org_id, client_id, name, host_ref) values ($1, $2, $3, $4) on conflict (org_id, host_ref) do nothing returning client_id) select client_id from added union all select client_id from works.clients where org_id = $1 and host_ref = $4";
