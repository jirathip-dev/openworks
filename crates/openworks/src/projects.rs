use openworks_core::{Call, Declaration, Fail, Id, Record, Row, Scope, StageKind, Tx, Value, Who, Word};
use openworks_db::events::SINCE;
use openworks_db::project_members::{ADD as ADD_MEMBER, CLEAR, LIST as MEMBERS_SQL};
use openworks_db::projects::{
    ADD, ARCHIVE as ARCHIVE_SQL, GET as GET_SQL, LINK as LINK_SQL, LIST as LIST_SQL, MOVE, SAVE as SAVE_SQL,
};
use openworks_db::{clients, jobs, lost_reasons, members, project_roles, series, sites, stages};

use crate::clients::{changed, lines, named, optional, org};
use crate::json;
use crate::tasks::count;

/// Lists a page of the projects the caller reaches from `stage`, `client_id`, `archived` (yes, no, or empty for both), `limit` (at most 500, 100 when empty) and `offset` lines, each empty filter off, as records of kind `projects`.
pub const LIST: Declaration = openworks_core::list!("list", needs: Read, record: "projects", queries: [LIST_SQL], list);

/// Reads the project with the body's id, as a page of one record of kind `projects`.
pub const GET: Declaration = openworks_core::list!("get", needs: Read, record: "projects", queries: [GET_SQL], get);

/// Reads the project with the body's id as one page: an object of its `project` record, its `members` and its `jobs`.
pub const PAGE: Declaration =
    openworks_core::read!("page", needs: Read, queries: [GET_SQL, MEMBERS_SQL, jobs::LIST], page);

/// Lists what changed after the sequence number on the body's first line, oldest first, at most `limit` (the second line) events, as records of kind `changes`; a caller who reads only their own projects is refused.
pub const CHANGES: Declaration =
    openworks_core::list!("changes", needs: Read, record: "changes", queries: [SINCE], changes);

/// Saves a project from `id`, `name`, `client_id`, `site_id`, `scope`, `tags` and `zone` lines, then `stage` when the empty id adds one with its first job, proposed; answers its id.
pub const SAVE: Declaration = openworks_core::create!(
    "save",
    needs: Write,
    queries: [GET_SQL, clients::GET, sites::GET, stages::GET, ADD, SAVE_SQL, jobs::ADD],
    save,
);

/// Moves a project from a body of `id`, `stage`, then `reason` and `note` lines when the stage is lost; winning opens and numbers its jobs.
pub const MOVE_STAGE: Declaration = openworks_core::write!(
    "move-stage",
    needs: Write,
    queries: [GET_SQL, stages::GET, lost_reasons::GET, MOVE, jobs::LIST, jobs::MOVE, series::NEXT],
    move_stage,
);

/// Links a project to a parent from a body of `id`, `parent_id` and `link_kind` lines; an empty parent unlinks it.
pub const LINK: Declaration = openworks_core::write!("link", needs: Write, queries: [GET_SQL, LINK_SQL], link);

/// Lists the people of the project with the body's id, as records of kind `project_members`.
pub const MEMBERS: Declaration = openworks_core::list!(
    "members",
    needs: Read,
    record: "project_members",
    queries: [GET_SQL, MEMBERS_SQL],
    list_members,
);

/// Replaces a project's people from a body of its id, then one `person role` line each.
pub const MEMBERS_SAVE: Declaration = openworks_core::write!(
    "members-save",
    needs: Write,
    queries: [GET_SQL, members::FIND, project_roles::GET, CLEAR, ADD_MEMBER],
    save_members,
);

/// Archives the project with the body's id.
pub const ARCHIVE: Declaration =
    openworks_core::write!("archive", needs: Write, queries: [GET_SQL, ARCHIVE_SQL], archive).by_hand();

const FIELDS: [&str; 14] = [
    "name",
    "client_id",
    "site_id",
    "scope",
    "tags",
    "zone",
    "stage",
    "parent_id",
    "link_kind",
    "won_at",
    "lost_at",
    "lost_reason",
    "lost_note",
    "archived_at",
];

const PAGE_SIZE: i64 = 100;
const MOST: i64 = 500;

const STAGE: usize = 7;
const PARENT: usize = 8;
const WON_AT: usize = 10;

fn list(who: &Who, tx: &mut dyn Tx, body: &str) -> Result<Vec<Record>, Fail> {
    let [stage, client, archived, limit, offset] = lines(body)?;
    let archived = match archived {
        "" => Value::Null,
        "no" => Value::Int(0),
        "yes" => Value::Int(1),
        _ => return Err(Fail::Invalid("archived is yes or no".to_owned())),
    };
    let [limit, offset] = paging(limit, offset)?;
    let params = [
        org(who),
        reaching(who, Word::Read),
        optional(stage),
        optional(client),
        archived,
        limit,
        offset,
    ];
    records(tx.run(LIST_SQL, &params)?, &FIELDS)
}

fn get(who: &Who, tx: &mut dyn Tx, body: &str) -> Result<Vec<Record>, Fail> {
    Ok(vec![record(project(who, tx, body.trim(), Word::Read)?, &FIELDS)?])
}

fn page(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let who = call.who;
    let id = call.body.trim();
    let found = record(project(who, tx, id, Word::Read)?, &FIELDS)?;
    let members = records(tx.run(MEMBERS_SQL, &[org(who), text(id)])?, &["role"])?;
    let jobs = records(tx.run(jobs::LIST, &[org(who), text(id)])?, &crate::jobs::FIELDS)?;
    Ok(json::sections([
        ("project", json::item(&found)),
        ("members", json::page(&members, &[])),
        ("jobs", json::page(&jobs, &[])),
    ]))
}

fn changes(who: &Who, tx: &mut dyn Tx, body: &str) -> Result<Vec<Record>, Fail> {
    if reaching(who, Word::Read) != Value::Null {
        return Err(Fail::Needs(Word::Read));
    }
    let [since, limit] = lines(body)?;
    let since = if since.is_empty() { 0 } else { count("since", since)? };
    let [limit, _] = paging(limit, "")?;
    records(
        tx.run(SINCE, &[org(who), Value::Int(since), limit])?,
        &["seq", "kind", "body", "at"],
    )
}

fn paging(limit: &str, offset: &str) -> Result<[Value; 2], Fail> {
    let limit = match limit {
        "" => PAGE_SIZE,
        limit => Some(count("limit", limit)?)
            .filter(|limit| (1..=MOST).contains(limit))
            .ok_or_else(|| Fail::Invalid(format!("limit is 1 to {MOST}")))?,
    };
    let offset = if offset.is_empty() { 0 } else { count("offset", offset)? };
    Ok([Value::Int(limit), Value::Int(offset)])
}

pub(crate) fn records(rows: Vec<Row>, fields: &[&'static str]) -> Result<Vec<Record>, Fail> {
    rows.into_iter().map(|row| record(row, fields)).collect()
}

fn save(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let who = call.who;
    let [id, name, client, site, scope, tags, zone, stage] = lines(call.body)?;
    let (client, site) = (optional(client), optional(site));
    place(who, tx, &client, &site)?;
    let saved = if id.is_empty() { (call.fresh)().0 } else { id.to_owned() };
    let mut params = vec![
        org(who),
        text(&saved),
        named(name)?,
        client,
        site,
        text(scope),
        Value::Text(tagged(tags)),
        text(zone),
    ];
    if !id.is_empty() {
        if !stage.is_empty() {
            return Err(Fail::Invalid("a project's stage changes by move-stage".to_owned()));
        }
        project(who, tx, id, Word::Write)?;
        return changed(tx.run(SAVE_SQL, &params)?);
    }
    if kind(who, tx, stage)? != StageKind::Open {
        return Err(Fail::Invalid("a project starts at an open stage".to_owned()));
    }
    params.push(text(stage));
    tx.run(ADD, &params)?;
    tx.run(jobs::ADD, &[org(who), text(&(call.fresh)().0), text(&saved)])?;
    Ok(saved)
}

fn move_stage(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let who = call.who;
    let [id, stage, reason, note] = lines(call.body)?;
    let row = project(who, tx, id, Word::Write)?;
    let from = kind(who, tx, &plain(row.get(STAGE)))?;
    let to = kind(who, tx, stage)?;
    if !from.moves_to(to) {
        return Err(Fail::Invalid(format!(
            "a project at a {} stage does not move to a {} stage",
            from.name(),
            to.name()
        )));
    }
    crate::jobs::follow(call, tx, id, from, to)?;
    let lost = if to == StageKind::Lost {
        lost(who, tx, reason, note, call.at)?
    } else if reason.is_empty() && note.is_empty() {
        [Value::Null, Value::Null, text("")]
    } else {
        return Err(Fail::Invalid("only a lost project has a reason".to_owned()));
    };
    let won_at = match (from, to) {
        (StageKind::Open, StageKind::Won) => Value::Int(call.at),
        (_, StageKind::Open) => Value::Null,
        _ => row.get(WON_AT).cloned().unwrap_or(Value::Null),
    };
    let [lost_at, lost_reason, lost_note] = lost;
    let params = [org(who), text(id), text(stage), won_at, lost_at, lost_reason, lost_note];
    changed(tx.run(MOVE, &params)?)
}

fn lost(who: &Who, tx: &mut dyn Tx, reason: &str, note: &str, at: i64) -> Result<[Value; 3], Fail> {
    if reason.is_empty() {
        return Err(Fail::Invalid("a lost project needs a reason".to_owned()));
    }
    let needs_note = match tx.run(lost_reasons::GET, &[org(who), text(reason)])?.first() {
        Some(row) => row.get(2) == Some(&Value::Int(1)),
        None => return Err(Fail::NotFound),
    };
    if needs_note && note.is_empty() {
        return Err(Fail::Invalid(format!("the reason {reason} needs a note")));
    }
    Ok([Value::Int(at), text(reason), text(note)])
}

fn link(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let who = call.who;
    let [id, parent, link_kind] = lines(call.body)?;
    project(who, tx, id, Word::Write)?;
    if parent.is_empty() != link_kind.is_empty() {
        return Err(Fail::Invalid("a link is a parent and a kind".to_owned()));
    }
    let mut above = parent.to_owned();
    while !above.is_empty() {
        if above == id {
            return Err(Fail::Invalid("a project cannot sit under itself".to_owned()));
        }
        above = plain(project(who, tx, &above, Word::Read)?.get(PARENT));
    }
    let params = [org(who), text(id), optional(parent), optional(link_kind)];
    changed(tx.run(LINK_SQL, &params)?)
}

fn list_members(who: &Who, tx: &mut dyn Tx, body: &str) -> Result<Vec<Record>, Fail> {
    let id = body.trim();
    project(who, tx, id, Word::Read)?;
    let rows = tx.run(MEMBERS_SQL, &[org(who), text(id)])?;
    rows.into_iter().map(|row| record(row, &["role"])).collect()
}

fn save_members(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let who = call.who;
    let mut body = call.body.lines().map(str::trim);
    let id = body.next().unwrap_or("");
    project(who, tx, id, Word::Write)?;
    let mut people = Vec::new();
    for line in body.filter(|line| !line.is_empty()) {
        let [person, role] = line.split_whitespace().collect::<Vec<_>>()[..] else {
            return Err(Fail::Invalid("a project member is a person and a role".to_owned()));
        };
        if tx.run(members::FIND, &[org(who), text(person)])?.is_empty() {
            return Err(Fail::Invalid(format!("{person} is not a member of the company")));
        }
        if tx.run(project_roles::GET, &[org(who), text(role)])?.is_empty() {
            return Err(Fail::Invalid(format!("no project role {role}")));
        }
        people.push([org(who), text(id), text(person), text(role)]);
    }
    tx.run(CLEAR, &[org(who), text(id)])?;
    for person in people {
        tx.run(ADD_MEMBER, &person)?;
    }
    Ok(id.to_owned())
}

fn archive(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let id = call.body.trim();
    project(call.who, tx, id, Word::Write)?;
    changed(tx.run(ARCHIVE_SQL, &[org(call.who), text(id), Value::Int(call.at)])?)
}

pub(crate) fn project(who: &Who, tx: &mut dyn Tx, id: &str, word: Word) -> Result<Row, Fail> {
    first(tx.run(GET_SQL, &[org(who), text(id), reaching(who, word)])?)
}

pub(crate) fn first(rows: Vec<Row>) -> Result<Row, Fail> {
    rows.into_iter().next().ok_or(Fail::NotFound)
}

pub(crate) fn reaching(who: &Who, word: Word) -> Value {
    match who.role.grants(word) {
        Some(Scope::Own) => text(&who.person.0),
        _ => Value::Null,
    }
}

fn place(who: &Who, tx: &mut dyn Tx, client: &Value, site: &Value) -> Result<(), Fail> {
    if *client != Value::Null {
        first(tx.run(clients::GET, &[org(who), client.clone()])?)?;
    }
    if *site != Value::Null && first(tx.run(sites::GET, &[org(who), site.clone()])?)?.get(1) != Some(client) {
        return Err(Fail::Invalid("a project's site is one of its client's".to_owned()));
    }
    Ok(())
}

fn kind(who: &Who, tx: &mut dyn Tx, stage: &str) -> Result<StageKind, Fail> {
    let row = first(tx.run(stages::GET, &[org(who), text(stage)])?)?;
    StageKind::named(&plain(row.get(2))).ok_or_else(|| Fail::Store("a stage row holds no kind".to_owned()))
}

fn tagged(tags: &str) -> String {
    let mut kept: Vec<&str> = Vec::new();
    for tag in tags.split(',').map(str::trim) {
        if !tag.is_empty() && !kept.contains(&tag) {
            kept.push(tag);
        }
    }
    kept.join(",")
}

pub(crate) fn plain(value: Option<&Value>) -> String {
    match value {
        Some(Value::Text(text)) => text.clone(),
        _ => String::new(),
    }
}

pub(crate) fn text(value: &str) -> Value {
    Value::Text(value.to_owned())
}

pub(crate) fn reopened(who: &Who, tx: &mut dyn Tx, id: &str) -> Result<(), Fail> {
    let row = project(who, tx, id, Word::Write)?;
    if kind(who, tx, &plain(row.get(STAGE)))? != StageKind::Done {
        return Ok(());
    }
    let won = tx
        .run(stages::LIST, &[org(who)])?
        .into_iter()
        .find(|stage| stage.get(2) == Some(&text(StageKind::Won.name())))
        .and_then(|stage| stage.into_iter().next())
        .ok_or_else(|| Fail::Invalid("the company has no won stage".to_owned()))?;
    let won_at = row.get(WON_AT).cloned().unwrap_or(Value::Null);
    let params = [org(who), text(id), won, won_at, Value::Null, Value::Null, text("")];
    changed(tx.run(MOVE, &params)?).map(drop)
}

pub(crate) fn record(row: Row, fields: &[&'static str]) -> Result<Record, Fail> {
    let mut values = row.into_iter();
    match values.next() {
        Some(Value::Text(id)) if values.len() == fields.len() => Ok(Record {
            id: Id(id),
            fields: fields.iter().copied().zip(values).collect(),
        }),
        _ => Err(Fail::Store(format!("a row is an id and {}", fields.join(", ")))),
    }
}
