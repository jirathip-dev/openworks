create table if not exists works.hour_kinds (
    org_id text not null references works.orgs (org_id),
    kind_key text not null,
    name text not null,
    sort bigint not null,
    primary key (org_id, kind_key)
);

create table if not exists works.daily_reports (
    org_id text not null references works.orgs (org_id),
    report_id text not null,
    job_id text not null,
    day text not null,
    number text not null,
    status text not null check (status in ('draft', 'submitted', 'approved', 'voided')),
    version bigint not null check (version > 0),
    primary key (org_id, report_id),
    unique (org_id, job_id, day),
    unique (org_id, number),
    foreign key (org_id, job_id) references works.jobs (org_id, job_id)
);

create table if not exists works.report_hours (
    org_id text not null references works.orgs (org_id),
    report_id text not null,
    worker_id text not null,
    task_id text not null,
    kind_key text not null,
    hours bigint not null check (hours > 0),
    primary key (org_id, report_id, worker_id, task_id, kind_key),
    foreign key (org_id, report_id) references works.daily_reports (org_id, report_id),
    foreign key (org_id, worker_id) references works.workers (org_id, worker_id),
    foreign key (org_id, task_id) references works.tasks (org_id, task_id),
    foreign key (org_id, kind_key) references works.hour_kinds (org_id, kind_key)
);

create table if not exists works.report_progress (
    org_id text not null references works.orgs (org_id),
    report_id text not null,
    task_id text not null,
    percent bigint not null check (percent between 0 and 100),
    primary key (org_id, report_id, task_id),
    foreign key (org_id, report_id) references works.daily_reports (org_id, report_id),
    foreign key (org_id, task_id) references works.tasks (org_id, task_id)
)
