create table if not exists works.equipment (
    org_id text not null references works.orgs (org_id),
    equipment_id text not null,
    tag text not null,
    name text not null,
    kind text not null default '',
    place text not null default '',
    holder_job_id text,
    status text not null default 'in_service' check (status in ('in_service', 'out_of_service')),
    primary key (org_id, equipment_id),
    unique (org_id, tag),
    foreign key (org_id, holder_job_id) references works.jobs (org_id, job_id)
);
create table if not exists works.equipment_bookings (
    org_id text not null references works.orgs (org_id),
    booking_id text not null,
    equipment_id text not null,
    job_id text not null,
    task_id text,
    starts_on text not null,
    ends_on text not null,
    status text not null default 'booked' check (status in ('booked', 'cancelled')),
    primary key (org_id, booking_id),
    foreign key (org_id, equipment_id) references works.equipment (org_id, equipment_id),
    foreign key (org_id, job_id) references works.jobs (org_id, job_id),
    foreign key (org_id, task_id) references works.tasks (org_id, task_id),
    check (starts_on <= ends_on)
);
create table if not exists works.equipment_needs (
    org_id text not null references works.orgs (org_id),
    need_id text not null,
    job_id text not null,
    task_id text,
    kind text not null,
    starts_on text not null,
    ends_on text not null,
    booking_id text,
    primary key (org_id, need_id),
    foreign key (org_id, job_id) references works.jobs (org_id, job_id),
    foreign key (org_id, task_id) references works.tasks (org_id, task_id),
    foreign key (org_id, booking_id) references works.equipment_bookings (org_id, booking_id),
    check (starts_on <= ends_on)
)
