create table if not exists works.attachments (
    org_id text not null references works.orgs (org_id),
    attachment_id text not null,
    tag text not null,
    name text not null,
    size bigint not null,
    at bigint not null,
    primary key (org_id, attachment_id)
);

create table if not exists works.series (
    org_id text not null references works.orgs (org_id),
    series_key text not null,
    period text not null,
    last bigint not null,
    primary key (org_id, series_key, period)
)
