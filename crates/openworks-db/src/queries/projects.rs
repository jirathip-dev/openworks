/// Lists a page of the projects at a stage, of a client and archived or not, each filter off when null: id, name, client, site, scope, tags, zone, stage, parent, link kind, won, lost, lost reason, lost note, archived.
pub const LIST: &str = "select p.project_id, p.name, p.client_id, p.site_id, p.scope, p.tags, p.zone, p.stage_key, p.parent_id, p.link_kind, p.won_at, p.lost_at, p.lost_reason, p.lost_note, p.archived_at from works.projects p where p.org_id = $1 and ($2::text is null or exists (select 1 from works.project_members m where m.org_id = p.org_id and m.project_id = p.project_id and m.person_id = $2)) and ($3::text is null or p.stage_key = $3) and ($4::text is null or p.client_id = $4) and ($5::bigint is null or (p.archived_at is not null) = ($5 = 1)) order by p.name, p.project_id limit $6 offset $7";

/// Reads one project, with the columns `LIST` reads.
pub const GET: &str = "select p.project_id, p.name, p.client_id, p.site_id, p.scope, p.tags, p.zone, p.stage_key, p.parent_id, p.link_kind, p.won_at, p.lost_at, p.lost_reason, p.lost_note, p.archived_at from works.projects p where p.org_id = $1 and p.project_id = $2 and ($3::text is null or exists (select 1 from works.project_members m where m.org_id = p.org_id and m.project_id = p.project_id and m.person_id = $3))";

/// Adds a project at a stage.
pub const ADD: &str = "insert into works.projects (org_id, project_id, name, client_id, site_id, scope, tags, zone, stage_key) values ($1, $2, $3, $4, $5, $6, $7, $8, $9)";

/// Changes a project's name, client, site, scope, tags and zone.
pub const SAVE: &str = "update works.projects set name = $3, client_id = $4, site_id = $5, scope = $6, tags = $7, zone = $8 where org_id = $1 and project_id = $2 returning project_id";

/// Moves a project to a stage with its won and lost marks.
pub const MOVE: &str = "update works.projects set stage_key = $3, won_at = $4, lost_at = $5, lost_reason = $6, lost_note = $7 where org_id = $1 and project_id = $2 returning project_id";

/// Links a project to a parent with a kind, or unlinks it with both null.
pub const LINK: &str = "update works.projects set parent_id = $3, link_kind = $4 where org_id = $1 and project_id = $2 returning project_id";

/// Archives a project, keeping the first time it was.
pub const ARCHIVE: &str = "update works.projects set archived_at = coalesce(archived_at, $3) where org_id = $1 and project_id = $2 returning project_id";

/// Adds the project a host knows by a reference, at a stage, unless it is there; returns its id either way.
pub const IMPORT: &str = "with added as (insert into works.projects (org_id, project_id, name, client_id, site_id, stage_key, host_ref) values ($1, $2, $3, $4, $5, $6, $7) on conflict (org_id, host_ref) do nothing returning project_id) select project_id from added union all select project_id from works.projects where org_id = $1 and host_ref = $7";
