create table if not exists works.members (
    org_id text not null references works.orgs (org_id),
    email text not null,
    person_id text,
    role text not null,
    primary key (org_id, email),
    unique (org_id, person_id)
);
