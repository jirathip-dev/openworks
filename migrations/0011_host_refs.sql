alter table works.sites add column if not exists host_ref text;
create unique index if not exists sites_host_ref on works.sites (org_id, host_ref);
alter table works.projects add column if not exists host_ref text;
create unique index if not exists projects_host_ref on works.projects (org_id, host_ref);
alter table works.jobs add column if not exists host_ref text;
create unique index if not exists jobs_host_ref on works.jobs (org_id, host_ref);
alter table works.tasks add column if not exists host_ref text;
create unique index if not exists tasks_host_ref on works.tasks (org_id, host_ref)
