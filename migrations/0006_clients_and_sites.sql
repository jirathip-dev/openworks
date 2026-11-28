create table if not exists works.clients (
    org_id text not null references works.orgs (org_id),
    client_id text not null,
    name text not null,
    host_ref text,
    deactivated_at bigint,
    primary key (org_id, client_id),
    unique (org_id, host_ref)
);

create table if not exists works.sites (
    org_id text not null,
    site_id text not null,
    client_id text not null,
    name text not null,
    address text not null default '',
    map_url text not null default '',
    deactivated_at bigint,
    primary key (org_id, site_id),
    foreign key (org_id, client_id) references works.clients (org_id, client_id)
)
