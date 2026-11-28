/// Reads the first answer to a create under its key.
pub const FIND: &str =
    "select answer from works.idempotency where org_id = $1 and operation = $2 and idempotency_key = $3";

/// Keeps a create's answer under its key.
pub const SAVE: &str =
    "insert into works.idempotency (org_id, operation, idempotency_key, answer, at) values ($1, $2, $3, $4, $5)";

/// Forgets an org's keys kept before a time.
pub const PRUNE: &str = "delete from works.idempotency where org_id = $1 and at < $2";
