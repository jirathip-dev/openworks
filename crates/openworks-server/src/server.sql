create schema if not exists server;

create table if not exists server.tokens (
    token_hash text primary key,
    org_id text not null references works.orgs (org_id),
    person_id text not null,
    email text not null,
    via text not null,
    created_at bigint not null
);

create table if not exists server.proposals (
    proposal_id text primary key,
    org_id text not null references works.orgs (org_id),
    person_id text not null,
    path text not null,
    body text not null,
    state text not null,
    at bigint not null
);
