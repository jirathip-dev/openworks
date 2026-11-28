/// Reads whether a company's own kind asks for a signature.
pub const KIND: &str = "select signature from works.approval_kinds where org_id = $1 and kind_key = $2";

/// Adds a company's own kind, or changes whether it asks for a signature.
pub const SAVE_KIND: &str = "insert into works.approval_kinds (org_id, kind_key, signature) values ($1, $2, $3) on conflict (org_id, kind_key) do update set signature = excluded.signature";

/// Asks for an approval unless one of its kind on its subject is pending or approved; returns its id when asked.
pub const REQUEST: &str = "insert into works.approvals (org_id, approval_id, kind_key, subject, state, asked_by, asked_at, asked_note) values ($1, $2, $3, $4, 'pending', $5, $6, $7) on conflict (org_id, kind_key, subject) where state in ('pending', 'approved') do nothing returning approval_id";

/// Reads the kind of a pending approval.
pub const OPEN: &str =
    "select kind_key from works.approvals where org_id = $1 and approval_id = $2 and state = 'pending'";

/// Records an assistant's proposed decision on a pending approval.
pub const PROPOSE: &str = "update works.approvals set proposed = $3, proposed_by = $4, proposed_note = $5 where org_id = $1 and approval_id = $2 and state = 'pending'";

/// Decides a pending approval: approved or rejected, by whom, when, the note and the signature file.
pub const DECIDE: &str = "update works.approvals set state = $3, decided_by = $4, decided_at = $5, decided_note = $6, signature = $7 where org_id = $1 and approval_id = $2 and state = 'pending'";

/// Resets an approved approval to none; returns its id when it was approved.
pub const RESET: &str = "update works.approvals set state = 'reset' where org_id = $1 and approval_id = $2 and state = 'approved' returning approval_id";

/// Lists the pending approvals, oldest first, with any assistant's proposal.
pub const PENDING: &str = "select approval_id, kind_key, subject, asked_by, asked_note, proposed, proposed_note from works.approvals where org_id = $1 and state = 'pending' order by asked_at, approval_id";

/// Lists the latest decided approvals, newest first.
pub const RECENT: &str = "select approval_id, kind_key, subject, state, decided_by, decided_note, signature from works.approvals where org_id = $1 and decided_at is not null order by decided_at desc, approval_id limit 50";
