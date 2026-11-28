# ADR: the four hot lists become collected folders

## Summary

Decision: The four files stop being lists. Each area's queries, core rules, operations and tests live in a file of their own inside a folder; a small build step (for tests, cargo itself) collects the folder. An area PR adds files and edits none of the four, which shrink to a few fixed lines and are frozen.
Slices: one PR, five commits, each green: (1) the shared collector and the database crate, (2) the core crate split by concern, (3) the app crate as a router plus an areas folder, (4) the router tests without per-area data, (5) a formatter gate for the collected folders and the `arch.toml` lines.
Open questions: one. Two "every operation" tests would check the role gate without per-operation sample data: refusals stay exact, "answers 200" is left to each area's own tests, which already call 43 of the 44 operations. My default is yes.

## Question

Four files took 9 to 11 of the last 16 merged PRs each, with 5 to 10 predicted conflicts and 2 or 3 update rounds. Twenty more issues are queued or in flight behind them. Why does every area PR edit these files, and what layout makes it stop?

Each file is a list that every area appends to, with other code around it:

| File | What an area PR appends |
|---|---|
| `crates/openworks-db/src/lib.rs` | a `MIGRATIONS` entry and a `pub mod` of queries |
| `crates/openworks/src/lib.rs` | a `mod` line, its `operations!` entries, its `HOMES` row, its `home()` arm, sometimes a host entry point |
| `crates/openworks-core/src/lib.rs` | its rule types, or a `mod` line and a `pub use` line for them |
| `crates/openworks/tests/router.rs` | a sample body per operation, canned rows per query, the migration statement count, sometimes its own tests |

## Options

1. **Keep the lists, one sorted line per entry.** Smallest change. Every area still edits the shared file, two PRs that add neighbouring lines still conflict, and a frozen file cannot take the line at all.
2. **Generate the lists and commit them** (`generated` in `arch.toml`). Tools see ordinary modules. Two PRs that each add a file both change the generated list, so the second still needs an update round to regenerate it.
3. **Collect the folders at build time.** A build script lists each folder and writes the module lines into cargo's output folder. Nothing shared is committed, so there is nothing to conflict on. Costs: a build script in three published crates, and `cargo fmt` no longer reaches the collected files.
4. **A registry or module-macro crate** (`inventory`, `automod`, `include_dir`). Does what option 3 does with two or three new dependencies.

For the test file no build step is needed: cargo already runs every `tests/*.rs` as its own target.

## Decision

Option 3 for the three `lib.rs` files, cargo's own collection for the tests. Shapes, moves and boundaries are in [`shapes.md`](shapes.md).

1. **One collector.** `build/collect.rs` lists a folder by extension in name order, writes `OUT_DIR/collected.rs` and tells cargo to rerun when the folder changes. Each of the three crates has its own short `build.rs` and a `collect.rs` symlink to it, as the database crate already links `migrations`. A copy per crate fails the duplication gate; the symlink passes it.
2. **Database crate.** `src/queries/<table>.rs` holds one table's SQL and is collected as `pub mod <table>`. `MIGRATIONS` is collected from `migrations/*.sql` in name order. `migrate` and `unscoped` move to `src/schema.rs`, `AUDIT` and `EVENT` to `src/trail.rs`. No public name changes.
3. **Core crate.** Every `src/*.rs` but `lib.rs` is collected as a private module whose public items are re-exported at the root. Today's `lib.rs` splits by concern into `who.rs`, `store.rs`, `declaration.rs`, `pack.rs` and `stage.rs`, beside `job.rs`. No public name changes, except that `operations!` is deleted.
4. **App crate.** `src/areas/<area>.rs` holds one area's operations and ends in `mount()`; the folder is collected as modules plus `AREAS`, the list of those functions. The router moves to `src/router.rs` with its machinery under `src/router/`. `Operation`, `HOMES` and `home()` are deleted: a mount already says which operations sit under which prefix, and `openworks::mounts()` gives tests the full list. A host entry point is an `impl Openworks` block in its area's file.
5. **Router tests.** `tests/router.rs` keeps the 18 tests of the router itself and stops knowing areas: `body()` and the canned rows go. The members tests move to `tests/members.rs`, the approvals tests to `tests/approvals.rs`. Four tests are restated so that no area can change them.
6. **Formatter gate.** `.gatekeeper/gates/fmt.toml` runs `rustfmt --check --edition 2024` on the three collected folders.
7. **Freeze.** The four files are frozen and their folders named as collected, in the same PR.

## Consequences

- An area PR touches no shared Rust file. The lists cannot conflict because they are not committed.
- A frozen file takes no new line. New code has a named place instead: a core concern is a new file in the core's `src/`; app machinery is a file under `src/router/`, declared in `router.rs`; a router test that must change moves to a new test file.
- `cargo fmt` skips the collected folders, which hold most of the source. The new gate fails an unformatted file and prints the command that formats it. Lanes that only run `cargo fmt` will meet that gate once.
- The module list of a crate is its folder listing, not its `lib.rs`. Editors need build scripts on, which is rust-analyzer's default.
- Three published crates gain a build script that uses only the standard library. The clippy gate lints it like any other code.
- In the core, every `pub` item of a file is public API. A helper is private or `pub(crate)`.
- A file in `src/areas/` without `mount()` does not compile. A declaration left out of its `mount()` is dead code, which the build reports.
- Migrations run in file-name order. Two PRs may take the same number; the number only orders. A migration that needs another area's table must sort after it.
- The restated role test no longer proves that every operation answers 200 to a granted role with canned rows; it proves that the role gate lets the role through. Each area's own tests carry the 200. `/projects/roles` is the one operation they do not call today, so slice 4 adds that call.
- Test targets of the app crate go from four to six.
- Any PR open when this merges needs one update round: what it appended to an old list becomes a file. Today that is #12 and #25. It lands cheapest between area PRs.
- Not converted, because the issue does not list them: the one-entry list of due jobs in `due.rs`, and the body helpers in `clients.rs` that four areas import. The same build script can collect a `due/` folder when a second due job arrives.

## `.drover/arch.toml`

The impl lane replaces the `frozen` line and adds four `[[collect]]` tables beside the three that exist. `migrations` is collected already.

```toml
frozen = [
    ".github/workflows/ci.yml",
    "crates/openworks-core/src/lib.rs",
    "crates/openworks-db/src/lib.rs",
    "crates/openworks/src/lib.rs",
    "crates/openworks/tests/router.rs",
]

[[collect]]
dir = "crates/openworks-core/src"
glob = "*.rs"

[[collect]]
dir = "crates/openworks-db/src/queries"
glob = "*.rs"

[[collect]]
dir = "crates/openworks/src/areas"
glob = "*.rs"

[[collect]]
dir = "crates/openworks/tests"
glob = "*.rs"
```
