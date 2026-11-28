use openworks_core::{Call, Declaration, Fail, Record, Row, Tx, Value, Who, Word};
use openworks_db::jobs::GET as JOB;
use openworks_db::plans::{
    GROUP_ADD, GROUP_SAVE, GROUPS as GROUPS_SQL, KEEP, LINK, TEMPLATE, TEMPLATE_ADD, TEMPLATE_SAVE,
    TEMPLATES as TEMPLATES_SQL, UNLINK, VERSION, VERSIONS as VERSIONS_SQL,
};
use openworks_db::tasks::{ADD, ARCHIVE, GET as TASK, LIST as TASKS, SAVE};

use crate::clients::{changed, lines, named, org};
use crate::projects::{first, plain, records, text};
use crate::tasks::{JOB_ID, job, line, put, retire, task};

/// Lists the groups of the job with the body's id in order, as records of kind `task_groups`.
pub const GROUPS: Declaration =
    openworks_core::list!("groups", needs: Read, record: "task_groups", queries: [JOB, GROUPS_SQL], groups);

/// Saves a job's groups in order from a `job_id` line, then one line per group of tab-separated `id` (empty adds one), `name` and its tasks' ids, comma-separated; answers their ids, a line each.
pub const GROUPS_SAVE: Declaration = openworks_core::create!(
    "groups-save",
    needs: Write,
    queries: [JOB, TASK, GROUP_ADD, GROUP_SAVE, UNLINK, LINK],
    save_groups,
);

/// Lists the numbered versions of the job with the body's id, oldest first, as records of kind `plan_versions`.
pub const VERSIONS: Declaration =
    openworks_core::list!("versions", needs: Read, record: "plan_versions", queries: [JOB, VERSIONS_SQL], versions);

/// Puts back the plan of the version with the body's id, keeping the plan it replaces as the job's next version; answers that version's id.
pub const RESTORE_VERSION: Declaration = openworks_core::create!(
    "restore-version",
    needs: Write,
    queries: [VERSION, JOB, TASKS, TASK, ADD, SAVE, ARCHIVE, VERSIONS_SQL, KEEP],
    restore_version,
);

/// Lists the company's plan templates by name, as records of kind `plan_templates`.
pub const TEMPLATES: Declaration =
    openworks_core::list!("templates", needs: Read, record: "plan_templates", queries: [TEMPLATES_SQL], templates);

/// Saves a plan template from `id` (empty adds one) and `name` lines, then one task line each as `tasks/save` takes them, with an empty id; answers its id.
pub const TEMPLATES_SAVE: Declaration = openworks_core::create!(
    "templates-save",
    needs: Write,
    queries: [TEMPLATE_ADD, TEMPLATE_SAVE],
    save_template,
);

/// Replaces the plan of a job from `job_id` and `template_id` lines, keeping the plan it replaces as the job's next version; answers that version's id.
pub const REPLACE_FROM_TEMPLATE: Declaration = openworks_core::create!(
    "replace-from-template",
    needs: Write,
    queries: [JOB, TEMPLATE, TASKS, TASK, ADD, SAVE, ARCHIVE, VERSIONS_SQL, KEEP],
    replace_from_template,
);

const PLAN: usize = 2;
const NUMBER: usize = 2;
const REPLACED_BY: usize = 4;
const VERSION_PLAN: usize = 5;
const ARCHIVED_AT: usize = 11;

fn groups(who: &Who, tx: &mut dyn Tx, body: &str) -> Result<Vec<Record>, Fail> {
    let id = body.trim();
    job(who, tx, id, Word::Read)?;
    records(
        tx.run(GROUPS_SQL, &[org(who), text(id)])?,
        &["job_id", "name", "order", "tasks"],
    )
}

fn save_groups(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let who = call.who;
    let mut body = call.body.lines();
    let job_id = body.next().unwrap_or("").trim();
    job(who, tx, job_id, Word::Write)?;
    let mut saved = Vec::new();
    for (order, line) in (1..).zip(body.filter(|line| !line.trim().is_empty())) {
        let fields: Vec<&str> = line.split('\t').map(str::trim).collect();
        let [id, name, tasks] = fields[..] else {
            return Err(Fail::Invalid("a group line is 3 tab-separated fields".to_owned()));
        };
        let adds = id.is_empty();
        let id = if adds { (call.fresh)().0 } else { id.to_owned() };
        let params = [org(who), text(&id), text(job_id), named(name)?, Value::Int(order)];
        if adds {
            tx.run(GROUP_ADD, &params)?;
        } else {
            changed(tx.run(GROUP_SAVE, &params)?)?;
        }
        tx.run(UNLINK, &[org(who), text(&id)])?;
        for task_id in tasks.split(',').map(str::trim).filter(|task_id| !task_id.is_empty()) {
            if plain(task(who, tx, task_id)?.get(JOB_ID)) != job_id {
                return Err(Fail::Invalid(format!("task {task_id} is not on job {job_id}")));
            }
            tx.run(LINK, &[org(who), text(&id), text(task_id)])?;
        }
        saved.push(id);
    }
    Ok(saved.join("\n"))
}

fn versions(who: &Who, tx: &mut dyn Tx, body: &str) -> Result<Vec<Record>, Fail> {
    let id = body.trim();
    job(who, tx, id, Word::Read)?;
    records(
        tx.run(VERSIONS_SQL, &[org(who), text(id)])?,
        &["job_id", "number", "came_from", "replaced_by", "plan", "at"],
    )
}

fn restore_version(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let version = first(tx.run(VERSION, &[org(call.who), text(call.body.trim())])?)?;
    let job_id = plain(version.get(1));
    job(call.who, tx, &job_id, Word::Write)?;
    let number = match version.get(NUMBER) {
        Some(Value::Int(number)) => *number,
        _ => return Err(Fail::Store("a version row holds no number".to_owned())),
    };
    replace(
        call,
        tx,
        &job_id,
        &plain(version.get(VERSION_PLAN)),
        &format!("version {number}"),
    )
}

fn templates(who: &Who, tx: &mut dyn Tx, _: &str) -> Result<Vec<Record>, Fail> {
    records(tx.run(TEMPLATES_SQL, &[org(who)])?, &["name", "plan"])
}

fn save_template(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let mut body = call.body.lines();
    let id = body.next().unwrap_or("").trim();
    let name = named(body.next().unwrap_or("").trim())?;
    let plan: Vec<&str> = body.filter(|line| !line.trim().is_empty()).collect();
    for task_line in &plan {
        if !line(task_line)?.id.is_empty() {
            return Err(Fail::Invalid("a template's task line leaves its id empty".to_owned()));
        }
    }
    let plan = text(&plan.join("\n"));
    if id.is_empty() {
        let id = (call.fresh)().0;
        tx.run(TEMPLATE_ADD, &[org(call.who), text(&id), name, plan])?;
        Ok(id)
    } else {
        changed(tx.run(TEMPLATE_SAVE, &[org(call.who), text(id), name, plan])?)
    }
}

fn replace_from_template(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let [job_id, template_id] = lines(call.body)?;
    job(call.who, tx, job_id, Word::Write)?;
    let template = first(tx.run(TEMPLATE, &[org(call.who), text(template_id)])?)?;
    replace(
        call,
        tx,
        job_id,
        &plain(template.get(PLAN)),
        &format!("template {template_id}"),
    )
}

fn replace(call: &Call, tx: &mut dyn Tx, job_id: &str, plan: &str, source: &str) -> Result<String, Fail> {
    let who = call.who;
    let incoming = plan.lines().map(line).collect::<Result<Vec<_>, _>>()?;
    let live: Vec<Row> = tx
        .run(TASKS, &[org(who), text(job_id)])?
        .into_iter()
        .filter(|row| row.get(ARCHIVED_AT) == Some(&Value::Null))
        .collect();
    let kept = tx.run(VERSIONS_SQL, &[org(who), text(job_id)])?;
    let number = kept.last().map_or(Ok(0), |last| match last.get(NUMBER) {
        Some(Value::Int(number)) => Ok(*number),
        _ => Err(Fail::Store("a version row holds no number".to_owned())),
    })? + 1;
    let came_from = kept
        .last()
        .and_then(|last| last.get(REPLACED_BY).cloned())
        .unwrap_or(Value::Null);
    let id = (call.fresh)().0;
    let outgoing: Vec<String> = live.iter().map(snapshot).collect();
    tx.run(
        KEEP,
        &[
            org(who),
            text(&id),
            text(job_id),
            Value::Int(number),
            came_from,
            text(source),
            text(&outgoing.join("\n")),
            Value::Int(call.at),
        ],
    )?;
    for row in &live {
        retire(call, tx, &plain(row.first()))?;
    }
    put(call, tx, job_id, incoming)?;
    Ok(id)
}

fn snapshot(row: &Row) -> String {
    let field = |at: usize| match row.get(at) {
        Some(Value::Text(value)) => value.clone(),
        Some(Value::Int(value)) => value.to_string(),
        _ => String::new(),
    };
    format!(
        "\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t\t{}",
        field(2),
        field(3),
        field(4),
        field(5),
        field(6),
        field(7),
        field(8),
        field(10)
    )
}
