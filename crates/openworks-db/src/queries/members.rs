/// Reads the role of the member tied to a person.
pub const FIND: &str = "select role from works.members where org_id = $1 and person_id = $2";

/// Ties the member added by an email to the person signing in, once.
pub const TIE: &str =
    "update works.members set person_id = $2 where org_id = $1 and email = $3 and person_id is null returning role";

/// Reads every member's email and role.
pub const LIST: &str = "select email, role from works.members where org_id = $1 order by email";

/// Reads the emails of the members who hold a role.
pub const HOLDING: &str = "select email from works.members where org_id = $1 and role = $2";

/// Adds a member by email, or changes their role.
pub const SET: &str = "insert into works.members (org_id, email, role) values ($1, $2, $3) on conflict (org_id, email) do update set role = excluded.role";

/// Removes a member by email.
pub const REMOVE: &str = "delete from works.members where org_id = $1 and email = $2";
