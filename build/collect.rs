use std::error::Error;
use std::io::{self, Write};
use std::path::Path;
use std::{env, fs};

/// A collected file: its name without the extension and its absolute path.
pub struct Entry {
    pub stem: String,
    pub path: String,
}

/// Lists the files of the crate's folder with the extension, by name, and tells cargo to rerun when it changes.
pub fn folder(dir: &str, extension: &str) -> Result<Vec<Entry>, Box<dyn Error>> {
    let root = Path::new(&env::var("CARGO_MANIFEST_DIR")?).join(dir);
    writeln!(io::stdout(), "cargo::rerun-if-changed={}", root.display())?;
    let mut entries = Vec::new();
    for item in fs::read_dir(&root)? {
        let path = item?.path();
        if path.extension().is_some_and(|found| found == extension) {
            let stem = path.file_stem().and_then(|stem| stem.to_str());
            let (Some(stem), Some(full)) = (stem, path.to_str()) else {
                return Err(format!("{} is not UTF-8", path.display()).into());
            };
            entries.push(Entry {
                stem: stem.to_owned(),
                path: full.to_owned(),
            });
        }
    }
    entries.sort_by(|one, other| one.stem.cmp(&other.stem));
    Ok(entries)
}

/// Writes the collected code to `OUT_DIR/collected.rs`.
pub fn emit(collected: &str) -> Result<(), Box<dyn Error>> {
    fs::write(Path::new(&env::var("OUT_DIR")?).join("collected.rs"), collected)?;
    Ok(())
}
