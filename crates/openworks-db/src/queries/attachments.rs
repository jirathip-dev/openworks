/// Records a file the host stored, under its tag.
pub const ADD: &str =
    "insert into works.attachments (org_id, attachment_id, tag, name, size, at) values ($1, $2, $3, $4, $5, $6)";

/// Finds a file by its id.
pub const FIND: &str = "select attachment_id from works.attachments where org_id = $1 and attachment_id = $2";

/// Lists the files under a tag, oldest first.
pub const LIST: &str =
    "select attachment_id, name, size from works.attachments where org_id = $1 and tag = $2 order by at, attachment_id";
