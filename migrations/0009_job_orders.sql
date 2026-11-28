create table if not exists works.job_orders (
    org_id text not null references works.orgs (org_id),
    job_order_id text not null,
    job_id text not null,
    revision bigint not null check (revision > 0),
    status text not null check (status in ('draft', 'issued', 'superseded')),
    issued_by text,
    issued_at bigint,
    primary key (org_id, job_order_id),
    unique (org_id, job_id, revision),
    foreign key (org_id, job_id) references works.jobs (org_id, job_id),
    check ((status = 'draft') = (issued_at is null)),
    check ((status = 'draft') = (issued_by is null))
);

create unique index if not exists job_orders_draft on works.job_orders (org_id, job_id) where status = 'draft';

create table if not exists works.job_order_lines (
    org_id text not null,
    job_order_id text not null,
    line bigint not null check (line > 0),
    body text not null,
    primary key (org_id, job_order_id, line),
    foreign key (org_id, job_order_id) references works.job_orders (org_id, job_order_id)
)
