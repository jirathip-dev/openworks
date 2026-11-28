# Shapes, moves and boundaries

No state changes: nothing here adds or alters a statechart. The core's statecharts stay in [`../core/statecharts.md`](../core/statecharts.md).

## The collector

`build/collect.rs`, linked into each crate as `collect.rs` and used by its `build.rs`:

```rust
pub struct Entry {
    pub stem: String,
    pub path: String,
}

pub fn folder(dir: &str, extension: &str) -> Result<Vec<Entry>, Box<dyn Error>>;

pub fn emit(collected: &str) -> Result<(), Box<dyn Error>>;
```

`folder` lists `CARGO_MANIFEST_DIR/<dir>` by extension, sorted by name, with absolute paths, and writes `cargo::rerun-if-changed=<dir>`. `emit` writes `OUT_DIR/collected.rs`. Both return errors; neither prints with `println!`.

## What each crate emits

Each `lib.rs` takes it in with `include!(concat!(env!("OUT_DIR"), "/collected.rs"));`.

Core, for every `src/*.rs` but `lib.rs`:

```rust
#[path = "<abs>/src/job.rs"]
mod job;
pub use job::*;
```

Database, for every `src/queries/*.rs`, then every `migrations/*.sql`:

```rust
#[path = "<abs>/src/queries/jobs.rs"]
pub mod jobs;

/// Every migration, by name, in the order it runs.
pub const MIGRATIONS: &[(&str, &str)] = &[
    ("0001_orgs", include_str!("<abs>/migrations/0001_orgs.sql")),
];
```

App, for every `src/areas/*.rs`:

```rust
#[path = "<abs>/src/areas/jobs.rs"]
mod jobs;

const AREAS: &[fn() -> openworks_core::Mount] = &[approvals::mount, clients::mount, jobs::mount];
```

Area modules sit at the crate root, so `crate::clients::lines` and the other paths areas use today do not change.

## The three frozen `lib.rs` files, whole

Core:

```rust
include!(concat!(env!("OUT_DIR"), "/collected.rs"));

#[cfg(doctest)]
#[doc = include_str!("../tests/undeclared.md")]
struct Undeclared;
```

Database:

```rust
include!(concat!(env!("OUT_DIR"), "/collected.rs"));

mod schema;
mod trail;

pub use schema::*;
pub use trail::*;
```

App:

```rust
include!(concat!(env!("OUT_DIR"), "/collected.rs"));

mod router;

pub use openworks_core::{Files, Via};
pub use router::*;
```

## The API that changes

| Item | Change |
|---|---|
| `openworks::Operation` with `ALL`, `declaration`, `home`, `path`, `mounts` | deleted |
| `openworks_core::operations!` | deleted |
| `openworks::mounts() -> Vec<Mount>` | new: every area's mount, in area-name order |
| `Mount::path(&self, declaration: &Declaration) -> String` | new: the prefix, a slash, the operation's name; `Builder::build` uses it too |
| `Openworks::as_host` | private to `pub(crate)`, so an area's host entry point can call it |
| `members::address` | moves into `router.rs` as `pub(crate)`: sign-in needs it and the router names no area |

Every other public name of the three crates keeps its path, among them `openworks_db::{MIGRATIONS, AUDIT, EVENT, migrate, unscoped}`, every `openworks_db::<table>::<QUERY>`, every `openworks_core` type and macro, `openworks::{Openworks, Builder, Host, Caller, Request, Response, Refused, Period, Series}`, `set_member` and `sync_client`.

## Where today's items go

`crates/openworks-core/src/lib.rs`:

| New file | Items |
|---|---|
| `who.rs` | `Id`, `Word`, `Scope`, `Role`, `Via`, `Who` |
| `store.rs` | `Value`, `Row`, `Type`, `Record`, `Fail`, `Tx`, `Files` |
| `declaration.rs` | `Call`, `Kind`, `Handler`, `Lister`, `Declaration`, and `read!`, `list!`, `write!`, `create!`, `closed!` |
| `pack.rs` | `Extender`, `Extension`, `Approval`, `Print`, `Pack`, `Change`, `Hook`, `Reach`, `Mount` |
| `stage.rs` | `StageKind` |
| `job.rs` | as it is |

`crates/openworks-db/src/lib.rs`:

| New file | Items |
|---|---|
| `queries/<table>.rs` | the body of each `pub mod <table>`: approvals, attachments, clients, idempotency, jobs, lost_reasons, members, orgs, project_members, project_roles, projects, series, sites, stages |
| `trail.rs` | `AUDIT`, `EVENT` |
| `schema.rs` | `migrate`, `unscoped` |
| nowhere | the hand-written `MIGRATIONS` list |

`crates/openworks/src/`:

| New place | Items |
|---|---|
| `router.rs` | `Caller`, `Host`, `Request`, `Response`, `Refused`, `Route`, `Builder`, `Openworks`, `Declared`, `status`, `answer`, `address`, the re-export of `Period` and `Series`, and `mounts()` |
| `router/` | `due.rs`, `due/keys.rs`, `idempotency.rs`, `json.rs`, `series.rs`, unchanged |
| `areas/` | `approvals.rs`, `clients.rs`, `files.rs`, `jobs.rs`, `members.rs`, `orgs.rs`, `projects.rs`, `setup.rs`, `sites.rs`, each with a `mount()` added |
| `areas/members.rs`, `areas/clients.rs` | `set_member`, `sync_client` |
| nowhere | `operations! { … }`, `HOMES`, `home()`, `path()` |

Two areas may share a prefix and key: `members.rs` mounts under `/orgs` beside `orgs.rs`, `setup.rs` under `/projects` beside `projects.rs`. `Builder::build` already refuses a path mounted twice.

`crates/openworks/tests/router.rs`, 28 tests:

| Goes to | Tests |
|---|---|
| stays, as it is | role table grants every word; test host mounts and answers; closed operation; trail names key and operation; failed write leaves nothing; unscoped query fails; every query is scoped; migrate gives events a sequence; both hook tests; both create tests; due work twice |
| stays, restated | the four below |
| stays, walks `openworks::mounts()` with its assertions kept | the OpenAPI document names every route |
| `tests/members.rs` | member added by email and tied at sign-in; a person who is no member is refused; a company always has an owner; a host sets members |
| `tests/approvals.rs` | the six approval tests with their rows and seed |
| deleted | `body()`, `job_rows`, `job`, the canned rows for clients, sites, projects and stages |

The four restated tests:

| Test | Today | Afterwards |
|---|---|---|
| the role table holds for every declared operation | each operation with its sample body: 200 or 403 | each operation of `openworks::mounts()` with an empty body: a role without the word gets 403, `needs <word>` and no statement; a role with it never gets `needs <word>` |
| every write leaves one audit row and one event | each operation with its sample body | one operation of each kind (read, write, create, closed) on the test's own `notes` mount |
| a path is mounted once | expects `/orgs/list` | mounts the first of `mounts()` again and expects its first path |
| migrate runs every statement and again does the same | expects 20 statements | expects the count of non-empty statements in `MIGRATIONS` |

## The gate file

`.gatekeeper/gates/fmt.toml`, on a leg that has Rust and runs where `rust/fmt` runs:

```toml
leg = "surface"
about = "rustfmt on the folders a build script collects, which cargo fmt does not reach"
fix = "run the gate's command without --check and commit the result"
budget_secs = 20
run = "rustfmt --check --edition 2024 crates/openworks-core/src/*.rs crates/openworks-db/src/queries/*.rs crates/openworks/src/areas/*.rs"
```

## Which module may call which

The crate layers stay as `arch.toml` has them: core, then database, then app, then server; packs over core only.

| Module | May use | Enforced by |
|---|---|---|
| a core file | any other core file, through `crate::` | the compiler |
| `queries/<table>.rs` | nothing: constants only | review against this ADR |
| `schema.rs`, `trail.rs` | core | the arch gate's layers |
| `router.rs`, `router/*` | core, database, and areas only through `AREAS` | review against this ADR |
| `areas/<area>.rs` | core, database, the router's `pub(crate)` items, another area's helpers as today | the compiler |
| `build.rs`, `collect.rs` | the standard library | the deps gate: a build dependency would need a listed reason |
| a frozen `lib.rs`, `tests/router.rs` | takes no new line | the arch gate's `frozen` |
| `tests/<area>.rs` | the public API, `tests/common`, `tests/memory` | the compiler |
