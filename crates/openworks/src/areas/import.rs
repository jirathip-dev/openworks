use std::collections::HashMap;

use openworks_core::{Call, Declaration, Fail, Id, JobStatus, Mount, Reach, Tx, Value};
use openworks_db::{clients, jobs, projects, sites, stages, tasks};

use crate::clients::{changed, named, optional, org};
use crate::jobs::{date, window};
use crate::projects::{first, text};
use crate::tasks::{count, lasting, planned};
use crate::{Host, Openworks};

/// Copies a host's existing records by `host_ref`, adding the missing and leaving the rest: one tab-separated line each, after the lines it names, of `client ref name`, `site ref client name address map_url`, `project ref name client site stage`, `job ref project status number starts_on ends_on` or `task ref job code name start_offset days planned_hours`; answers `kind ref id` lines.
pub const IMPORT: Declaration = openworks_core::write!(
    "import",
    needs: Settings,
    queries: [stages::GET, clients::IMPORT, sites::IMPORT, projects::IMPORT, jobs::IMPORT, tasks::IMPORT],
    import,
);

#[derive(Default)]
struct Spine<'a> {
    ids: HashMap<(&'a str, &'a str), String>,
    site_clients: HashMap<&'a str, &'a str>,
    sorts: HashMap<&'a str, i64>,
}

fn import(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let mut spine = Spine::default();
    let mut answer = Vec::new();
    for line in call.body.lines().filter(|line| !line.trim().is_empty()) {
        let fields: Vec<&str> = line.split('\t').map(str::trim).collect();
        let [kind, reference, ref fields @ ..] = fields[..] else {
            return Err(Fail::Invalid(
                "an import line is a kind, a host_ref and its fields".to_owned(),
            ));
        };
        if reference.is_empty() {
            return Err(Fail::Invalid(
                "an import line is a kind, a host_ref and its fields".to_owned(),
            ));
        }
        let (sql, params) = match (kind, fields) {
            ("client", &[name]) => (clients::IMPORT, vec![named(name)?]),
            ("site", &[client, name, address, map_url]) => {
                spine.site_clients.insert(reference, client);
                let params = vec![spine.id("client", client)?, named(name)?, text(address), text(map_url)];
                (sites::IMPORT, params)
            }
            ("project", &[name, client, site, stage]) => {
                first(tx.run(stages::GET, &[org(call.who), text(stage)])?)?;
                (projects::IMPORT, spine.project(name, client, site, stage)?)
            }
            ("job", &[project, status, number, starts_on, ends_on]) => {
                let [status, number, starts_on, ends_on] = job(status, number, starts_on, ends_on)?;
                (
                    jobs::IMPORT,
                    vec![spine.id("project", project)?, status, number, starts_on, ends_on],
                )
            }
            ("task", &[job, code, name, offset, days, hours]) => {
                (tasks::IMPORT, spine.task(job, code, name, offset, days, hours)?)
            }
            _ => return Err(Fail::Invalid(format!("an import line of kind {kind} is out of shape"))),
        };
        let params = [
            vec![org(call.who), text(&(call.fresh)().0)],
            params,
            vec![text(reference)],
        ]
        .concat();
        let saved = changed(tx.run(sql, &params)?)?;
        answer.push(format!("{kind}\t{reference}\t{saved}"));
        spine.ids.insert((kind, reference), saved);
    }
    Ok(answer.join("\n"))
}

impl<'a> Spine<'a> {
    fn id(&self, kind: &str, reference: &str) -> Result<Value, Fail> {
        self.ids
            .get(&(kind, reference))
            .map(|id| text(id))
            .ok_or_else(|| Fail::Invalid(format!("no {kind} {reference} before this line")))
    }

    fn project(&self, name: &str, client: &str, site: &str, stage: &str) -> Result<Vec<Value>, Fail> {
        if !site.is_empty() && self.site_clients.get(site) != Some(&client) {
            return Err(Fail::Invalid(format!("site {site} is not one of client {client}'s")));
        }
        let client = if client.is_empty() {
            Value::Null
        } else {
            self.id("client", client)?
        };
        let site = if site.is_empty() {
            Value::Null
        } else {
            self.id("site", site)?
        };
        Ok(vec![named(name)?, client, site, text(stage)])
    }

    fn task(
        &mut self,
        job: &'a str,
        code: &str,
        name: &str,
        offset: &str,
        days: &str,
        hours: &str,
    ) -> Result<Vec<Value>, Fail> {
        let sort = self.sorts.entry(job).or_insert(0);
        *sort += 1;
        let sort = Value::Int(*sort);
        let offset = if offset.is_empty() {
            0
        } else {
            count("start_offset", offset)?
        };
        Ok(vec![
            self.id("job", job)?,
            text(code),
            named(name)?,
            Value::Int(offset),
            lasting(days)?,
            planned(hours)?,
            sort,
        ])
    }
}

fn job(status: &str, number: &str, starts_on: &str, ends_on: &str) -> Result<[Value; 4], Fail> {
    let numbered = match JobStatus::named(status) {
        Some(JobStatus::Proposed) => number.is_empty(),
        Some(_) => !number.is_empty(),
        None => return Err(Fail::Invalid(format!("no job status {status}"))),
    };
    if !numbered {
        return Err(Fail::Invalid(
            "a job has a number once it is no longer proposed".to_owned(),
        ));
    }
    if starts_on.is_empty() && ends_on.is_empty() {
        return Ok([text(status), optional(number), Value::Null, Value::Null]);
    }
    let planned = window(date(starts_on)?, date(ends_on)?)?;
    let [starts_on, ends_on] = [planned.starts_on(), planned.ends_on()].map(|day| text(&day.to_string()));
    Ok([text(status), optional(number), starts_on, ends_on])
}

pub(crate) fn mount() -> Mount {
    Mount::new("/projects", "projects").operation(&IMPORT, Reach::Org)
}

impl<H: Host> Openworks<H> {
    /// Copies the records a host already keeps, by `host_ref`, on the host's word; running it twice writes what once did.
    pub fn import(&self, by: Id, org: Id, records: &str) -> Result<String, Fail> {
        self.as_host(by, org, &IMPORT, "projects", records)
    }
}
