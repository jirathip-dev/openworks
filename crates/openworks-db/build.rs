use std::error::Error;
use std::fmt::Write;

mod collect;

fn main() -> Result<(), Box<dyn Error>> {
    let mut collected = String::new();
    for query in collect::folder("src/queries", "rs")? {
        writeln!(collected, "#[path = {:?}]\npub mod {};", query.path, query.stem)?;
    }
    collected
        .push_str("/// Every migration, by name, in the order it runs.\npub const MIGRATIONS: &[(&str, &str)] = &[\n");
    for migration in collect::folder("migrations", "sql")? {
        writeln!(
            collected,
            "    ({:?}, include_str!({:?})),",
            migration.stem, migration.path
        )?;
    }
    collected.push_str("];\n");
    collect::emit(&collected)
}
