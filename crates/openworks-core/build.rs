use std::error::Error;
use std::fmt::Write;

mod collect;

fn main() -> Result<(), Box<dyn Error>> {
    let mut collected = String::new();
    for file in collect::folder("src", "rs")? {
        if file.stem != "lib" {
            writeln!(
                collected,
                "#[path = {:?}]\nmod {};\npub use {}::*;",
                file.path, file.stem, file.stem
            )?;
        }
    }
    collect::emit(&collected)
}
