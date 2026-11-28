create table if not exists works.tasks (
    org_id text not null references works.orgs (org_id),
    task_id text not null,
    job_id text not null,
    code text not null default '',
    name text not null,
    color text,
    start_offset bigint not null default 0 check (start_offset >= 0),
    days bigint not null default 1 check (days >= 1),
    planned_hours bigint check (planned_hours >= 0),
    weight bigint check (weight >= 0),
    sort bigint not null,
    remark text not null default '',
    archived_at bigint,
    primary key (org_id, task_id),
    foreign key (org_id, job_id) references works.jobs (org_id, job_id)
)
