# Shapes

The `works` schema, the core types, the host trait, extensions and hooks, and the declaration. Why: [`decisions.md`](decisions.md). Statecharts: [`statecharts.md`](statecharts.md). Rust and SQL here are sketches of the shape, not the final spelling.

## The `works` schema

Every table has `org_id uuid not null`, an `id uuid` (UUIDv7) and, where a person edits it, `version integer` for the stale-write check. Every foreign key includes `org_id`. `host_ref text`, unique per company where present, is the id a host already knows the row by. No column holds money.

### Foundation (before any slice)

```sql
create table works.orgs (
  id uuid primary key, name text not null,
  time_zone text not null,                 -- an IANA name; the company's day
  work_days smallint[] not null            -- ISO weekdays that count by default (a job may differ)
);
create table works.members (
  org_id uuid, user_id uuid, email text not null,
  role text not null check (role in ('owner','manager','member','viewer')),
  active boolean not null default true,
  primary key (org_id, user_id)
);
create table works.events (org_id uuid, seq bigint, kind text, at timestamptz, by uuid, via text, payload jsonb, primary key (org_id, seq));
create table works.audit_log (org_id uuid, id uuid, at timestamptz, by uuid, via text, action text, subject text, subject_id uuid, before jsonb, after jsonb);
create table works.idempotency_keys (org_id uuid, key text, body_hash text, answer jsonb, primary key (org_id, key));
create table works.number_series (org_id uuid, key text, pattern text, period text, next bigint, primary key (org_id, key, period));
create table works.attachments (org_id uuid, id uuid, subject text, subject_id uuid, file_key text, name text, tag text, size bigint);
```

### Slice 1: the spine

```sql
create table works.clients (
  org_id uuid, id uuid, name text not null, host_ref text,
  primary key (org_id, id), unique (org_id, host_ref)
);
create table works.sites (
  org_id uuid, id uuid, client_id uuid not null, name text not null,
  address text not null default '', map_url text not null default '', host_ref text, version integer not null,
  primary key (org_id, id), foreign key (org_id, client_id) references works.clients
);
create table works.stages (                 -- the company's own list
  org_id uuid, key text, name text not null,
  kind text not null check (kind in ('open','won','done','lost')), sort integer not null,
  primary key (org_id, key)
);
create table works.lost_reasons (org_id uuid, key text, name text not null, needs_note boolean not null, primary key (org_id, key));
create table works.member_roles (org_id uuid, key text, name text not null, primary key (org_id, key));

create table works.projects (
  org_id uuid, id uuid, host_ref text,
  name text not null, client_id uuid, site_id uuid,   -- a new lead may have neither yet
  scope text not null default '', tags text[] not null default '{}', zone text,
  stage text not null,                      -- → works.stages
  parent_id uuid, link_kind text,           -- a project linked to a main one; the kinds are the company's
  won_at timestamptz, lost_at timestamptz, lost_reason text, lost_note text not null default '',
  archived_at timestamptz, version integer not null,
  created_by uuid, created_at timestamptz, updated_by uuid, updated_at timestamptz,
  primary key (org_id, id), unique (org_id, host_ref),
  check ((parent_id is null) = (link_kind is null)), check (parent_id is distinct from id)
);
create table works.project_members (
  org_id uuid, project_id uuid, user_id uuid, role text not null,   -- → works.member_roles
  primary key (org_id, project_id, user_id, role)
);
create table works.jobs (
  org_id uuid, id uuid, project_id uuid not null, host_ref text,
  number text,                              -- given when the job opens, from works.number_series; unique per company, never reused
  status text not null check (status in ('proposed','open','finished','cancelled')),
  at_site boolean not null default true,    -- false: workshop work, which needs no site window
  base_date date, work_days smallint[],     -- the plan's day 1 and which weekdays count
  actual_start date, actual_end date, remark text not null default '',
  finished_at timestamptz, version integer not null,
  primary key (org_id, id), unique (org_id, number)
);
create table works.job_windows (            -- the planned window, then any further ranges
  org_id uuid, job_id uuid, seq integer, starts_on date not null, ends_on date not null, kind text not null check (kind in ('planned','actual')),
  primary key (org_id, job_id, kind, seq), check (starts_on <= ends_on)
);
create table works.tasks (
  org_id uuid, id uuid, job_id uuid not null, host_ref text,
  code text not null default '', name text not null, color text,
  start_offset integer not null default 0, days integer not null default 1,
  planned_hours numeric(9,2), weight numeric(9,4),
  origin text not null check (origin in ('planned','added')),
  sort integer not null, remark text not null default '', archived_at timestamptz, version integer not null,
  primary key (org_id, id)
);
```

### Later slices

| Slice                      | Tables                                                                                                               | Holds                                                                                                                                                                                                                                                                                                        |
| -------------------------- | -------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 2 planner                  | `task_groups`, `task_group_links`, `plan_versions`, `plan_templates`                                                 | groups over tasks; a numbered snapshot of a plan with what it came from; reusable plans                                                                                                                                                                                                                      |
| 3 crew and equipment       | `teams`, `workers`, `crew_seats`, `crew_days`, `equipment`, `equipment_bookings`, `equipment_needs`                  | a worker (name, title, team, active, own staff or an outside company's, an optional user); a seat on a job (title, the worker in it, their number); the days a seat works; a bookable thing (tag, name, kind, place, holder job, status); a booking of it for a job or task and dates; a need not yet booked |
| 4 daily reports            | `hour_kinds`, `daily_reports`, `report_hours`, `report_progress`, `report_weather`, `report_issues`, `report_photos` | the company's kinds of hour (regular, overtime, …); one report per job and day with its number, status and version; hours per worker, task and kind; declared progress per task; weather intervals; issues raised; photos per task                                                                           |
| 5 job orders and approvals | `job_orders`, `job_order_lines`, `approval_kinds`, `approvals`                                                       | the instruction issued for a job, by revision, with its lines; the kinds of approval a company or a pack has; one row per request with who asked, who decided, the note and the signature file                                                                                                               |
| 6 Today                    | none                                                                                                                 | reads only                                                                                                                                                                                                                                                                                                   |

Reads that are not tables: the planner board (jobs with windows, crew, clashes and issues in one answer), a worker's availability, equipment clashes, a job's hours by day, a project's page.

## Core types

```rust
pub struct OrgId(Uuid);  pub struct ProjectId(Uuid);  pub struct JobId(Uuid);  pub struct TaskId(Uuid);  // one newtype per id

/// A company's stage. The rules read `kind` only.
pub struct Stage { pub key: StageKey, pub name: String, pub kind: StageKind }
pub enum StageKind { Open, Won, Done, Lost }

pub enum JobStatus { Proposed, Open, Finished, Cancelled }
pub enum Role { Owner, Manager, Member, Viewer }
/// What an operation needs. A role is a fixed set of these.
pub enum Can { Read, Report, Plan, Approve, Own }

/// Whole days and hundredths of an hour. There is no money type in this crate.
pub struct Days(u32);  pub struct Hours(i64);

pub struct Window { pub starts_on: Date, pub ends_on: Date }      // cannot be built with starts_on > ends_on
pub struct JobCalendar { pub base_date: Date, pub work_days: WorkDays }
```

| Role                    | Read | Report | Plan | Approve | Own |
| ----------------------- | ---- | ------ | ---- | ------- | --- |
| Owner                   | yes  | yes    | yes  | yes     | yes |
| Manager                 | yes  | yes    | yes  | yes     |     |
| Member (their projects) | yes  | yes    |      |         |     |
| Viewer                  | yes  |        |      |         |     |

Read: any read. Report: a daily report, hours, progress, a photo. Plan: projects, jobs, tasks, crew, bookings, job orders. Approve: decide an approval, approve a report. Own: members, stages, setup.

## The host trait

Five answers, with a scope that fits projects:

```rust
pub trait Host: Clone + Send + Sync + 'static {
    /// Who is signed in. The host has already checked the request's credentials.
    async fn who(&self, request: &http::request::Parts) -> Result<SignedIn, Refusal>;
    /// Runs `work` in one transaction for this person. The host opens it and decides commit or rollback.
    async fn transact<T: Send>(&self, who: &SignedIn, work: Work<'_, T>) -> Result<T, Error>;
    fn new_id(&self) -> Uuid { Uuid::now_v7() }
    fn now(&self) -> Timestamp { Timestamp::now() }
    fn files(&self) -> &dyn Files;
    async fn names(&self, users: &[UserId]) -> Result<Vec<(UserId, String)>, Error>;
}

pub struct SignedIn { pub org: OrgId, pub user: UserId, pub email: Option<VerifiedEmail>, pub scope: Scope, pub via: Via }

/// A host may narrow. `Own` is the projects the caller is a member of.
pub enum Scope { All, Own }
pub enum Via { Person, Assistant, Host(&'static str), Pack(&'static str), Schedule }

/// Where the host mounts the router. `reach` is the host's own gate per operation, checked before the core's role.
pub struct Mount { /* at(prefix), declared_as(key), reach(fn(Operation) -> Reach) */ }
```

`Operation` is an enum generated from the declarations, so a host's reach map is a `match` that stops compiling when the core adds an operation.

## Extensions and hooks

```rust
/// A pack's own data beside a core record. Called inside the read's transaction, once per page.
pub trait Extension: Send + Sync {
    fn key(&self) -> &'static str;                     // "x" in `ext.x`
    fn subject(&self) -> Subject;                      // Project, Job, Task, Worker, Equipment, DailyReport
    fn schema(&self) -> Schema;                        // joins the OpenAPI document, so the client is typed
    async fn attach(&self, tx: &mut PgConnection, caller: &Caller, ids: &[Uuid]) -> Result<HashMap<Uuid, Value>, Error>;
}

/// A change about to commit, with the rows it wrote.
pub enum Change {
    ProjectCreated(Project), ProjectChanged(Project), StageMoved { project: Project, from: Stage, to: Stage },
    JobOpened(Job), JobNumbered(Job), JobRescheduled(Job), JobFinished(Job), TaskChanged(Task), TaskArchived(Task),
    SiteChanged(Site), MembersChanged(ProjectId),
    PlanReplaced(PlanVersion), CrewChanged(JobId), BookingChanged(Booking),                    // slices 2, 3
    ReportSubmitted(DailyReport), ReportApproved(DailyReport), ReportVoided(DailyReport),     // slice 4
    JobOrderIssued(JobOrder), ApprovalRequested(Approval), ApprovalDecided(Approval),         // slice 5
}

pub trait Hooks: Send + Sync {
    async fn before_commit(&self, tx: &mut PgConnection, caller: &Caller, change: &Change) -> Result<(), Refusal> { Ok(()) }
    async fn after_commit(&self, event: &Event) -> Result<(), HookError> { Ok(()) }
}

/// A kind of approval a pack adds. Its subject is the pack's own record, named by a reference.
pub struct ApprovalKind { pub key: &'static str, pub name: &'static str, pub needs: Can, pub signature: bool }
```

A host's own work comes in through the same functions the routes use, as `Via::Host("name")`:

```rust
works.clients().sync(&caller, &[HostClient { host_ref, name }])            // upsert by host_ref
works.projects().create(&caller, NewProject { host_ref, name, client, site, stage, .. })   // with its first job, proposed
works.projects().move_stage(&caller, project, StageKey::new("quoted"), expected_version)
works.approvals().request(&caller, "pack_kind", HostSubject::new(subject_id), note)
works.import(&caller, Spine { clients, sites, projects, jobs, tasks, members })   // the cut-over's copy: by host_ref, run twice, written once
```

## The declaration

Every operation declares `Declared` (`Read`, `Write { class, proposal }`, `Closed(why)`), plus `needs`, one of the five words. **T** a read tool, **P** a proposal, **H** by hand only.

| Area                                   | Operations                                                                                                                                                  |
| -------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `clients`, `sites`                     | `list`, `get` T · `save`, `set_active` P                                                                                                                    |
| `projects`                             | `list`, `get`, `page`, `changes` (what changed since a sequence number) T · `create`, `save`, `move_stage`, `link`, `members_save` P · `archive` H (delete) |
| `jobs`                                 | `list`, `get`, `board` T · `open`, `save`, `reschedule`, `finish`, `reopen` P · `next_number` T · `cancel` H (delete)                                       |
| `tasks`                                | `list` T · `save` (one or many, in order), `archive`, `restore` P                                                                                           |
| `plan` (slice 2)                       | `groups`, `versions`, `templates`, `issues`, `clashes` T · `groups_save`, `replace_from_template`, `restore_version`, `templates_save` P                    |
| `workers`, `teams` (slice 3)           | `list`, `availability` T · `save`, `set_active` P                                                                                                           |
| `crew` (slice 3)                       | `get` T · `seats_save`, `days_save` P                                                                                                                       |
| `equipment` (slice 3)                  | `list`, `timeline`, `clashes` T · `save`, `book`, `cancel_booking`, `need`, `cancel_need` P                                                                 |
| `daily_reports` (slice 4)              | `list`, `get`, `hours_by_day`, `print` T · `save`, `submit`, `approve`, `restore` P · `photo_add` H (file) · `void`, `discard` H (delete)                   |
| `job_orders` (slice 5)                 | `list`, `get`, `print` T · `save`, `issue` P                                                                                                                |
| `approvals` (slice 5)                  | `pending`, `recent` T · `request`, `decide` P · `reset` H (admin)                                                                                           |
| `today` (slice 6)                      | `get` T                                                                                                                                                     |
| `members`, `setup`, `audit`, `exports` | `list` / `get` T where it holds no personal contact · every write H (admin)                                                                                 |

A person's name is `PersonName`; a phone or email is `Contact` (an assistant gets an id); a photo or a file is never in a tool's answer.

