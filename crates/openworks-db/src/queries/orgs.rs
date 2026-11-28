/// Lists the orgs the caller reads: id and name.
pub const LIST: &str = "select org_id, name from works.orgs where org_id = $1 order by org_id";

/// Renames the caller's org.
pub const RENAME: &str = "update works.orgs set name = $2 where org_id = $1";

/// Lists every org, for due work.
pub const ALL: &str = "select org_id from works.orgs";
