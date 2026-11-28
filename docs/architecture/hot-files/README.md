# Hot files: the four lists become folders

## Summary

Decision: The four files stop being lists. Each area's queries, core rules, operations and tests live in a file of their own inside a folder; a small build step (for tests, cargo itself) collects the folder. An area PR adds files and edits none of the four, which shrink to a few fixed lines and are frozen.
Slices: one PR, five commits, each green: (1) the shared collector and the database crate, (2) the core crate split by concern, (3) the app crate as a router plus an areas folder, (4) the router tests without per-area data, (5) a formatter gate for the collected folders and the `arch.toml` lines.
Open questions: one. Two "every operation" tests would check the role gate without per-operation sample data: refusals stay exact, "answers 200" is left to each area's own tests, which already call 43 of the 44 operations. My default is yes.

## Files here

- [`adr.md`](adr.md): the question, the options, the decision, its consequences, the `arch.toml` lines.
- [`shapes.md`](shapes.md): what the build step emits, the API that changes, where every item moves, which module may call which.

## How the code reads afterwards

A new area, say visits, is five new files and no edit to a shared one.

| New file | Holds | Read as |
|---|---|---|
| `migrations/0009_visits.sql` | its tables | `openworks_db::MIGRATIONS` |
| `crates/openworks-db/src/queries/visits.rs` | its SQL | `openworks_db::visits::LIST` |
| `crates/openworks-core/src/visit.rs` | its rules, if it has any | `openworks_core::VisitStatus` |
| `crates/openworks/src/areas/visits.rs` | its operations and its mount | `/visits/list` |
| `crates/openworks/tests/visits.rs` | its tests | a test target of its own |

The queries file is the body of today's `pub mod visits { … }`, one indent less:

```rust
/// Lists a job's visits: id, job and day.
pub const LIST: &str = "select visit_id, job_id, day from works.visits where org_id = $1 and job_id = $2 order by day";
```

The area file declares its operations as today and ends with its mount, written the way a pack writes one:

```rust
/// Lists the visits of the job with the body's id, as records of kind `visits`.
pub const LIST: Declaration = openworks_core::list!("list", needs: Read, record: "visits", queries: [LIST_SQL], list);

pub(crate) fn mount() -> Mount {
    Mount::new("/visits", "visits").operation(&LIST, Reach::Org)
}
```

An entry point for the host lives in its area's file, not in the router:

```rust
impl<H: Host> Openworks<H> {
    /// Adds the client the host knows by `host_ref`, or renames it, on the host's word; answers the client's id.
    pub fn sync_client(&self, by: Id, org: Id, host_ref: &str, name: &str) -> Result<String, Fail> {
        self.as_host(by, org, &SYNC, "clients", &format!("{host_ref}\n{name}"))
    }
}
```

The host that embeds openworks does not change: `Openworks::new(host).pack(pack).build()`.

A test that walks every operation asks for the mounts instead of the old `Operation` enum:

```rust
for mount in openworks::mounts() {
    for (declaration, _) in mount.operations() {
        let path = mount.path(declaration);
        let refused = send(&openworks, &path, "viewer@o.test", Some("k"), "");
    }
}
```

## Layout

```
build/collect.rs                      the one collector: lists a folder, writes OUT_DIR/collected.rs
crates/openworks-core/
  build.rs  collect.rs -> build/      every src/*.rs but lib.rs: `mod x; pub use x::*;`
  src/lib.rs                          frozen: the include and the doctest
  src/who.rs store.rs declaration.rs pack.rs stage.rs job.rs
crates/openworks-db/
  build.rs  collect.rs -> build/      src/queries/*.rs: `pub mod x;`  migrations/*.sql: MIGRATIONS
  src/lib.rs                          frozen: the include, two mods, two re-exports
  src/schema.rs  src/trail.rs         migrate, unscoped; AUDIT, EVENT
  src/queries/<table>.rs              14 files today
crates/openworks/
  build.rs  collect.rs -> build/      src/areas/*.rs: `mod x;` and AREAS
  src/lib.rs                          frozen: the include, one mod, two re-exports
  src/router.rs                       today's lib.rs from `Caller` down, plus `mounts()`
  src/router/                         due.rs, due/keys.rs, idempotency.rs, json.rs, series.rs
  src/areas/<area>.rs                 9 files today, each ending in `mount()`
  tests/router.rs                     frozen: the 18 tests of the router itself
  tests/members.rs  tests/approvals.rs   the 10 tests that leave router.rs
.gatekeeper/gates/fmt.toml            rustfmt on the collected folders
```

## Slices

One PR. Each commit builds and passes the suite.

1. **Collector and database crate.** `build/collect.rs`, the database crate's `build.rs`, `src/queries/`, `schema.rs`, `trail.rs`. Public names do not change.
2. **Core crate.** `lib.rs` split into the six files of [`shapes.md`](shapes.md). Public names do not change.
3. **App crate.** `router.rs`, `router/`, `areas/`, a `mount()` per area, `openworks::mounts()`, `Mount::path`. `Operation` and `operations!` are deleted; `set_member` and `sync_client` move to their areas.
4. **Router tests.** The moves and the four restated tests of [`shapes.md`](shapes.md); `body()` and the canned rows are deleted; one call of `/projects/roles` is added to `tests/projects.rs`.
5. **Gate and freeze.** `.gatekeeper/gates/fmt.toml` and the `arch.toml` lines that end [`adr.md`](adr.md).

## What was run

In a scratch clone at `e7f31ca`, with the database crate converted and the core's `job.rs` collected:

| Run | Result |
|---|---|
| `cargo test --workspace` | every test passes; `openworks-db/src/lib.rs` goes from 290 lines to 34 |
| `cargo package --workspace --locked`, with its verify build | builds from the packed crates; the database crate packs `build.rs`, `collect.rs`, `migrations/` and `src/queries/` |
| duplication gate, collector copied into three crates | FAIL: 26 duplicated lines over the ceiling of 0 |
| duplication gate, collector shared by symlink | ok: 0 duplicated lines |
| arch gate on the converting commit, with the freeze lines | ok |
| arch gate on a later commit that adds a line to a frozen `lib.rs` and a file to `src/queries/` | FAIL on the line, nothing on the file |
| `cargo fmt --check` with a badly formatted collected file | exit 0: it does not see the file |
| `rustfmt --check --edition 2024` on that file, alone and as a local gate | exit 1, FAIL: the gate closes the hole |
| dead-code, complexity, test-health and docs gates | ok |
| clippy gate | lints build scripts too: they return errors and write `cargo::` lines with `writeln!` to stdout, never `println!` |

Not run: the app crate's conversion (its mechanism, modules plus a list of `mount` functions, was run in a toy crate, where a declaration left out of the list raised the dead-code warning), and the publish dry run's registry query.
