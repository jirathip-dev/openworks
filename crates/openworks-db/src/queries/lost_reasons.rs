/// Lists every lost reason: key, name and whether it needs a note.
pub const LIST: &str =
    "select reason_key, name, needs_note from works.lost_reasons where org_id = $1 order by reason_key";

/// Reads one lost reason: key, name and whether it needs a note.
pub const GET: &str =
    "select reason_key, name, needs_note from works.lost_reasons where org_id = $1 and reason_key = $2";

/// Adds a lost reason by key, or changes its name and whether it needs a note.
pub const SAVE: &str = "insert into works.lost_reasons (org_id, reason_key, name, needs_note) values ($1, $2, $3, $4) on conflict (org_id, reason_key) do update set name = excluded.name, needs_note = excluded.needs_note";
