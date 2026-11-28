create table if not exists works.seats (
    org_id text not null references works.orgs (org_id),
    seat_id text not null,
    job_id text not null,
    title text not null,
    worker_id text,
    number bigint check (number >= 1),
    primary key (org_id, seat_id),
    unique (org_id, job_id, number),
    foreign key (org_id, job_id) references works.jobs (org_id, job_id),
    foreign key (org_id, worker_id) references works.workers (org_id, worker_id)
);

create table if not exists works.seat_days (
    org_id text not null references works.orgs (org_id),
    seat_id text not null,
    day text not null,
    primary key (org_id, seat_id, day),
    foreign key (org_id, seat_id) references works.seats (org_id, seat_id)
)
