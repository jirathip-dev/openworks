create table if not exists works.teams (
    org_id text not null references works.orgs (org_id),
    team_id text not null,
    name text not null,
    primary key (org_id, team_id)
);

create table if not exists works.workers (
    org_id text not null references works.orgs (org_id),
    worker_id text not null,
    name text not null,
    title text not null default '',
    team_id text,
    company text,
    person_id text,
    active bigint not null default 1 check (active in (0, 1)),
    primary key (org_id, worker_id),
    unique (org_id, person_id),
    foreign key (org_id, team_id) references works.teams (org_id, team_id)
)
