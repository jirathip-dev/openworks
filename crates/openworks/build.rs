use std::error::Error;
use std::fmt::Write;

mod collect;

fn main() -> Result<(), Box<dyn Error>> {
    let areas = collect::folder("src/areas", "rs")?;
    let mut collected = String::new();
    for area in &areas {
        writeln!(collected, "#[path = {:?}]\nmod {};", area.path, area.stem)?;
    }
    let mounts: Vec<String> = areas.iter().map(|area| format!("{}::mount", area.stem)).collect();
    writeln!(
        collected,
        "const AREAS: &[fn() -> openworks_core::Mount] = &[{}];",
        mounts.join(", ")
    )?;
    collect::emit(&collected)
}
