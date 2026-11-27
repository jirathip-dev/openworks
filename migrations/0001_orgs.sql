create schema if not exists works;

create table if not exists works.orgs (
    org_id text primary key,
    name text not null,
    created_at bigint not null
);

create table if not exists works.audit (
    audit_id text primary key,
    org_id text not null references works.orgs (org_id),
    person_id text not null,
    operation text not null,
    at bigint not null
);

create table if not exists works.events (
    event_id text primary key,
    org_id text not null references works.orgs (org_id),
    kind text not null,
    body text not null,
    at bigint not null
);
