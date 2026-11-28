create table if not exists works.stages (
    org_id text not null references works.orgs (org_id),
    stage_key text not null,
    name text not null,
    kind text not null check (kind in ('open', 'won', 'done', 'lost')),
    sort bigint not null default 0,
    primary key (org_id, stage_key)
);

create table if not exists works.lost_reasons (
    org_id text not null references works.orgs (org_id),
    reason_key text not null,
    name text not null,
    needs_note bigint not null default 0 check (needs_note in (0, 1)),
    primary key (org_id, reason_key)
);

create table if not exists works.project_roles (
    org_id text not null references works.orgs (org_id),
    role_key text not null,
    name text not null,
    primary key (org_id, role_key)
);

create table if not exists works.projects (
    org_id text not null references works.orgs (org_id),
    project_id text not null,
    name text not null,
    client_id text,
    site_id text,
    scope text not null default '',
    tags text not null default '',
    zone text not null default '',
    stage_key text not null,
    parent_id text,
    link_kind text,
    won_at bigint,
    lost_at bigint,
    lost_reason text,
    lost_note text not null default '',
    archived_at bigint,
    primary key (org_id, project_id),
    foreign key (org_id, client_id) references works.clients (org_id, client_id),
    foreign key (org_id, site_id) references works.sites (org_id, site_id),
    foreign key (org_id, stage_key) references works.stages (org_id, stage_key),
    foreign key (org_id, parent_id) references works.projects (org_id, project_id),
    foreign key (org_id, lost_reason) references works.lost_reasons (org_id, reason_key),
    check ((parent_id is null) = (link_kind is null)),
    check (parent_id is distinct from project_id)
);

create table if not exists works.project_members (
    org_id text not null,
    project_id text not null,
    person_id text not null,
    role_key text not null,
    primary key (org_id, project_id, person_id, role_key),
    foreign key (org_id, project_id) references works.projects (org_id, project_id),
    foreign key (org_id, role_key) references works.project_roles (org_id, role_key)
)
