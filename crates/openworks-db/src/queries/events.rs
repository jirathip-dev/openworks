/// Lists the events after a sequence number, oldest first, at most a page: id, sequence number, kind, body, time.
pub const SINCE: &str =
    "select event_id, seq, kind, body, at from works.events where org_id = $1 and seq > $2 order by seq limit $3";
