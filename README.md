# openworks

Open field and site work: sites, projects, jobs, planner, crews, daily reports.

## Intent

- One open core for field and site work: who works where, on what, with which crew and equipment, and what was done.
- Any trade, any country: the core names none; a pack brings a trade's words, fields and screens.
- Every write is accountable: each one leaves one audit row and one event, and every query on a `works` table is
  scoped by `org_id`.
- Runs anywhere: the core names no host; the suite passes with no pack installed.

## Non-goals

- No money: no prices, costs, rates, budgets or invoices. No column holds money.
- No payroll: hours are recorded, never paid.
- No purchasing: no orders to suppliers, no stock.
- No trade-specific data in the core: that lives in a pack.

## Layout

| Path | What |
|---|---|
| `crates/openworks-core` | the domain types and rules |
| `crates/openworks-db` | storage, over the core |
| `crates/openworks` | the library a host embeds |
| `crates/openworks-server` | the standalone server |
| `packs/example` | an example pack, never published |
| `web` | the web package |
| `migrations` | the database migrations |
| `templates` | the print and message templates |

## Checks

`./.gatekeeper/run check` runs every gate (packs core, rust and ts), the same locally, in a lane and in CI. The
layers and bans live in `.drover/arch.toml`: no self-hosted runner in a workflow, no money word in the schema or the
code, no trade word in the core, the database crate or a migration. CI is `.github/workflows/ci.yml` alone, on
GitHub-hosted runners, with read-only permissions; a fork's pull request gets no secret.

## Branch protection

On `main` and `staging`:

- Require a pull request before merging; no direct pushes, no force pushes, no deletions.
- Require status checks to pass, with `gates / done` as the required check, and the branch up to date.
- Apply the rules to administrators too.

## License

Apache-2.0
