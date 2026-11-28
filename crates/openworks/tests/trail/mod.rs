use openworks_core::{Role, Row, Value};
use openworks_db::{AUDIT, EVENT, jobs, members};

/// A text value.
pub fn text(value: &str) -> Value {
    Value::Text(value.to_owned())
}

/// What a store keeps of the trail: the audit rows it counted and the kind of each event.
#[derive(Clone, Default)]
pub struct Trail {
    pub audits: usize,
    pub events: Vec<Value>,
}

impl Trail {
    /// Answers the sign-in of a person named for their role, the trail and `job_row`; else `None`.
    pub fn run(&mut self, sql: &str, p: &[Value]) -> Option<Vec<Row>> {
        Some(match sql {
            members::FIND => match &p[1] {
                Value::Text(person) => Role::named(person).map_or_else(Vec::new, |role| vec![vec![text(role.name())]]),
                _ => Vec::new(),
            },
            AUDIT => {
                self.audits += 1;
                Vec::new()
            }
            EVENT => {
                self.events.push(p[2].clone());
                Vec::new()
            }
            jobs::GET => job_row(&p[1], &p[2]),
            _ => return None,
        })
    }
}

/// The row `jobs::GET` reads of job `j1` or `j2` of `p1`, for a person staffed on `j1` only.
pub fn job_row(id: &Value, person: &Value) -> Vec<Row> {
    let staffed = *person == Value::Null || *id == text("j1");
    if !staffed || ![text("j1"), text("j2")].contains(id) {
        return Vec::new();
    }
    let mut row = vec![
        id.clone(),
        text("p1"),
        text(&format!("J-2026-{}", if *id == text("j1") { 1 } else { 2 })),
    ];
    row.resize(13, Value::Null);
    vec![row]
}
