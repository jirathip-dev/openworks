A declared operation mounts:

```rust
use openworks_core::{Fail, Mount, Reach, Who};

fn whoami(who: &Who, _: &str) -> Result<String, Fail> {
    Ok(who.person.0.clone())
}

const WHOAMI: openworks_core::Declaration =
    openworks_core::closed!("whoami", needs: "orgs.read", whoami);

let _ = Mount::new("/x", "x").operation(&WHOAMI, Reach::Org);
```

The same handler with no declaration does not compile:

```compile_fail,E0308
use openworks_core::{Fail, Mount, Reach, Who};

fn whoami(who: &Who, _: &str) -> Result<String, Fail> {
    Ok(who.person.0.clone())
}

let _ = Mount::new("/x", "x").operation(&whoami, Reach::Org);
```

Nor does a declaration without the word it needs:

```compile_fail
use openworks_core::{Fail, Who};

fn whoami(who: &Who, _: &str) -> Result<String, Fail> {
    Ok(who.person.0.clone())
}

const WHOAMI: openworks_core::Declaration = openworks_core::closed!("whoami", whoami);
```
