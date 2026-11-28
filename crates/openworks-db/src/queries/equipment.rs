/// Lists every bookable thing by tag: id, tag, name, kind, place, holder job, status.
pub const LIST: &str = "select equipment_id, tag, name, kind, place, holder_job_id, status from works.equipment where org_id = $1 order by tag, equipment_id";

/// Reads one thing, with the columns `LIST` reads, locking it until the transaction ends.
pub const LOCK: &str = "select equipment_id, tag, name, kind, place, holder_job_id, status from works.equipment where org_id = $1 and equipment_id = $2 for update";

/// Adds a thing with its tag, name, kind, place, holder job and status.
pub const ADD: &str = "insert into works.equipment (org_id, equipment_id, tag, name, kind, place, holder_job_id, status) values ($1, $2, $3, $4, $5, $6, $7, $8)";

/// Changes a thing's tag, name, kind, place, holder job and status.
pub const SAVE: &str = "update works.equipment set tag = $3, name = $4, kind = $5, place = $6, holder_job_id = $7, status = $8 where org_id = $1 and equipment_id = $2 returning equipment_id";

/// Lists the live bookings of a thing that meet the days from `$3` to `$4`.
pub const CLASHES: &str = "select booking_id from works.equipment_bookings where org_id = $1 and equipment_id = $2 and status = 'booked' and starts_on <= $4 and ends_on >= $3";

/// Books a thing for a job, or a task of it, from one day to another.
pub const BOOK: &str = "insert into works.equipment_bookings (org_id, booking_id, equipment_id, job_id, task_id, starts_on, ends_on, status) values ($1, $2, $3, $4, $5, $6, $7, 'booked')";

/// Reads one booking: id, thing, job, task, first and last day, status.
pub const BOOKING: &str = "select booking_id, equipment_id, job_id, task_id, starts_on, ends_on, status from works.equipment_bookings where org_id = $1 and booking_id = $2";

/// Cancels a live booking; returns its id when it was live.
pub const CANCEL: &str = "update works.equipment_bookings set status = 'cancelled' where org_id = $1 and booking_id = $2 and status = 'booked' returning booking_id";

/// Lists the live bookings that meet the days from `$2` to `$3`, thing by thing by tag, then by first day: id, thing, job, task, first and last day.
pub const TIMELINE: &str = "select b.booking_id, b.equipment_id, b.job_id, b.task_id, b.starts_on, b.ends_on from works.equipment_bookings b join works.equipment e on e.org_id = b.org_id and e.equipment_id = b.equipment_id join works.jobs j on j.org_id = b.org_id and j.job_id = b.job_id where b.org_id = $1 and b.status = 'booked' and b.starts_on <= $3 and b.ends_on >= $2 and ($4::text is null or exists (select 1 from works.project_members m where m.org_id = j.org_id and m.project_id = j.project_id and m.person_id = $4)) order by e.tag, b.starts_on, b.booking_id";

/// Adds a need of a kind of thing for a job, or a task of it, from one day to another.
pub const NEED: &str = "insert into works.equipment_needs (org_id, need_id, job_id, task_id, kind, starts_on, ends_on) values ($1, $2, $3, $4, $5, $6, $7)";

/// Reads one need: id, job, task, kind, first and last day, the booking that fills it.
pub const GET_NEED: &str = "select need_id, job_id, task_id, kind, starts_on, ends_on, booking_id from works.equipment_needs where org_id = $1 and need_id = $2";

/// Lists the needs no booking fills, by first day: id, job, task, kind, first and last day.
pub const NEEDS: &str = "select n.need_id, n.job_id, n.task_id, n.kind, n.starts_on, n.ends_on from works.equipment_needs n join works.jobs j on j.org_id = n.org_id and j.job_id = n.job_id where n.org_id = $1 and n.booking_id is null and ($2::text is null or exists (select 1 from works.project_members m where m.org_id = j.org_id and m.project_id = j.project_id and m.person_id = $2)) order by n.starts_on, n.need_id";

/// Fills a need no booking fills with a booking; returns its id when it was open.
pub const FILL: &str = "update works.equipment_needs set booking_id = $3 where org_id = $1 and need_id = $2 and booking_id is null returning need_id";

/// Opens the need a booking filled again.
pub const RELEASE: &str = "update works.equipment_needs set booking_id = null where org_id = $1 and booking_id = $2";
