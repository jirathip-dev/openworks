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

/// Adds the site a host knows by a reference, under a client, unless it is there; returns its id either way.
pub const IMPORT: &str = "with added as (insert into works.sites (org_id, site_id, client_id, name, address, map_url, host_ref) values ($1, $2, $3, $4, $5, $6, $7) on conflict (org_id, host_ref) do nothing returning site_id) select site_id from added union all select site_id from works.sites where org_id = $1 and host_ref = $7";
