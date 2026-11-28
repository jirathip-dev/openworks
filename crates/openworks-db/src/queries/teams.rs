/// Lists the org's teams by name: id and name.
pub const LIST: &str = "select team_id, name from works.teams where org_id = $1 order by name, team_id";

/// Reads one team, with the columns `LIST` reads.
pub const GET: &str = "select team_id, name from works.teams where org_id = $1 and team_id = $2";

/// Adds a team with its name.
pub const ADD: &str = "insert into works.teams (org_id, team_id, name) values ($1, $2, $3)";

/// Renames a team.
pub const SAVE: &str = "update works.teams set name = $3 where org_id = $1 and team_id = $2 returning team_id";
