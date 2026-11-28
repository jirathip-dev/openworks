use openworks_core::{Approval, Extension, Fail, Hook, Id, Mount, Pack, Print, Reach, Tx, Type, Value, Who, Word};

/// Counts the changes in each org's event log.
pub const CHANGES: &str = "select org_id, count(*) from works.events where org_id = $1 group by org_id";

pub(crate) const EXTENSION: Extension = Extension {
    record: "orgs",
    key: "example",
    fields: &[("changes", Type::Int)],
    queries: &[CHANGES],
    extend: changes,
};

/// Answers who is calling, under `/example/whoami`.
pub const WHOAMI: openworks_core::Declaration =
    openworks_core::closed!("whoami", needs: Read, |who, _| Ok(who.person.0.clone()));

/// Refuses a change whose body is longer than this many bytes.
pub const LONGEST: usize = 4096;

/// The example pack: one extension, hook, approval kind, route and print.
pub fn pack() -> Pack {
    Pack {
        extensions: vec![&EXTENSION],
        hooks: vec![Box::new(Bounded)],
        approvals: vec![Approval {
            key: "example.handover",
            needs: Word::Approve,
        }],
        mounts: vec![Mount::new("/example", "example").operation(&WHOAMI, Reach::Org)],
        prints: vec![Print {
            key: "example.summary",
            record: "orgs",
            template: "{{name}}: {{ext.example.changes}} changes",
        }],
    }
}

struct Bounded;

impl Hook for Bounded {
    fn before_commit(&self, change: &openworks_core::Change) -> Result<(), Fail> {
        if change.body.len() > LONGEST {
            return Err(Fail::Invalid(format!("a change body is at most {LONGEST} bytes")));
        }
        Ok(())
    }
}

fn changes(who: &Who, tx: &mut dyn Tx, _: &[Id]) -> Result<Vec<(Id, Vec<Value>)>, Fail> {
    let rows = tx.run(CHANGES, &[Value::Text(who.org.0.clone())])?;
    rows.into_iter()
        .map(|row| match <[Value; 2]>::try_from(row) {
            Ok([Value::Text(org), count]) => Ok((Id(org), vec![count])),
            _ => Err(Fail::Store("a change count row is its org and count".to_owned())),
        })
        .collect()
}
