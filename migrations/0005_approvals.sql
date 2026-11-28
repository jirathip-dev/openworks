create table if not exists works.approval_kinds (
    org_id text not null references works.orgs (org_id),
    kind_key text not null,
    signature text not null check (signature in ('yes', 'no')),
    primary key (org_id, kind_key)
);

create table if not exists works.approvals (
    org_id text not null references works.orgs (org_id),
    approval_id text not null,
    kind_key text not null,
    subject text not null,
    state text not null check (state in ('pending', 'approved', 'rejected', 'reset')),
    asked_by text not null,
    asked_at bigint not null,
    asked_note text not null,
    proposed text check (proposed in ('yes', 'no')),
    proposed_by text,
    proposed_note text,
    decided_by text,
    decided_at bigint,
    decided_note text,
    signature text,
    primary key (org_id, approval_id),
    foreign key (org_id, signature) references works.attachments (org_id, attachment_id)
);

create unique index if not exists approvals_live on works.approvals (org_id, kind_key, subject)
    where state in ('pending', 'approved')
