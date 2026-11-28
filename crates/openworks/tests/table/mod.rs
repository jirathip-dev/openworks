use openworks_core::{Role, Row, Value};
use openworks_db::{AUDIT, EVENT, members};

/// A text value.
pub fn text(value: &str) -> Value {
    Value::Text(value.to_owned())
}

/// The rows a hit picks, without their `org_id`.
pub fn select(table: &[Row], hit: impl Fn(&Row) -> bool) -> Vec<Row> {
    table
        .iter()
        .filter(|row| hit(row))
        .map(|row| row[1..].to_vec())
        .collect()
}

/// Changes the first row a hit picks and answers its id, or nothing.
pub fn change(table: &mut [Row], hit: impl Fn(&Row) -> bool, set: impl FnOnce(&mut Row)) -> Vec<Row> {
    table.iter_mut().find(|row| hit(row)).map_or_else(Vec::new, |row| {
        set(row);
        vec![vec![row[1].clone()]]
    })
}

/// The sign-in, audit and event queries: a person named for a role holds it, anyone else is an Owner.
#[derive(Clone, Default)]
pub struct Trail {
    pub events: Vec<(Value, Value)>,
    pub audits: usize,
}

impl Trail {
    /// Answers the query if it is one of the trail's, else none.
    pub fn run(&mut self, sql: &str, p: &[Value]) -> Option<Vec<Row>> {
        Some(match sql {
            members::FIND => {
                let role = match &p[1] {
                    Value::Text(person) => Role::named(person),
                    _ => None,
                };
                vec![vec![text(role.unwrap_or(Role::Owner).name())]]
            }
            AUDIT => {
                self.audits += 1;
                Vec::new()
            }
            EVENT => {
                self.events.push((p[2].clone(), p[3].clone()));
                Vec::new()
            }
            _ => return None,
        })
    }
}
