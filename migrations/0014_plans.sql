create table if not exists works.task_groups (
    org_id text not null references works.orgs (org_id),
    group_id text not null,
    job_id text not null,
    name text not null,
    sort bigint not null,
    primary key (org_id, group_id),
    foreign key (org_id, job_id) references works.jobs (org_id, job_id)
);
create table if not exists works.task_group_links (
    org_id text not null references works.orgs (org_id),
    group_id text not null,
    task_id text not null,
    primary key (org_id, group_id, task_id),
    foreign key (org_id, group_id) references works.task_groups (org_id, group_id),
    foreign key (org_id, task_id) references works.tasks (org_id, task_id)
);
create table if not exists works.plan_templates (
    org_id text not null references works.orgs (org_id),
    template_id text not null,
    name text not null,
    plan text not null,
    primary key (org_id, template_id)
);
create table if not exists works.plan_versions (
    org_id text not null references works.orgs (org_id),
    version_id text not null,
    job_id text not null,
    number bigint not null,
    came_from text,
    replaced_by text not null,
    plan text not null,
    at bigint not null,
    primary key (org_id, version_id),
    unique (org_id, job_id, number),
    foreign key (org_id, job_id) references works.jobs (org_id, job_id)
)
