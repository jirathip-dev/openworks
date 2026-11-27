alter table works.events add column if not exists seq bigint generated always as identity unique;

create table if not exists works.idempotency (
    org_id text not null references works.orgs (org_id),
    operation text not null,
    idempotency_key text not null,
    answer text not null,
    at bigint not null,
    primary key (org_id, operation, idempotency_key)
);
