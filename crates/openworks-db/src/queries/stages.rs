/// Lists every stage: key, name, kind and sort.
pub const LIST: &str =
    "select stage_key, name, kind, sort from works.stages where org_id = $1 order by sort, stage_key";

/// Reads one stage: key, name, kind and sort.
pub const GET: &str = "select stage_key, name, kind, sort from works.stages where org_id = $1 and stage_key = $2";

/// Adds a stage by key, or changes its name, kind and sort.
pub const SAVE: &str = "insert into works.stages (org_id, stage_key, name, kind, sort) values ($1, $2, $3, $4, $5) on conflict (org_id, stage_key) do update set name = excluded.name, kind = excluded.kind, sort = excluded.sort";
