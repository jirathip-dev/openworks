/// Lists a job's seats by number: id, job, title, worker and number.
pub const LIST: &str = "select seat_id, job_id, title, worker_id, number from works.seats where org_id = $1 and job_id = $2 order by number, seat_id";

/// Reads one seat, with the columns `LIST` reads.
pub const GET: &str =
    "select seat_id, job_id, title, worker_id, number from works.seats where org_id = $1 and seat_id = $2";

/// Adds a seat to a job with its title, worker and number.
pub const ADD: &str =
    "insert into works.seats (org_id, seat_id, job_id, title, worker_id, number) values ($1, $2, $3, $4, $5, $6)";

/// Changes a seat's title, worker and number.
pub const SAVE: &str = "update works.seats set title = $4, worker_id = $5, number = $6 where org_id = $1 and seat_id = $2 and job_id = $3 returning seat_id";

/// Lists the days a seat works.
pub const DAYS: &str = "select day from works.seat_days where org_id = $1 and seat_id = $2 order by day";

/// Lists the days a worker works in the job's other seats than the one bound.
pub const HELD: &str = "select d.day from works.seat_days d join works.seats s on s.org_id = d.org_id and s.seat_id = d.seat_id where d.org_id = $1 and s.job_id = $2 and s.worker_id = $3 and s.seat_id <> $4 order by d.day";

/// Removes every day of a seat.
pub const CLEAR: &str = "delete from works.seat_days where org_id = $1 and seat_id = $2";

/// Adds a day a seat works.
pub const ADD_DAY: &str = "insert into works.seat_days (org_id, seat_id, day) values ($1, $2, $3)";

/// Lists the seat days from one day to another across jobs not cancelled, by day: seat, job, worker and day; a person bound in place of null narrows it to the jobs of the projects they are a member of.
pub const BOOKED: &str = "select s.seat_id, s.job_id, s.worker_id, d.day from works.seat_days d join works.seats s on s.org_id = d.org_id and s.seat_id = d.seat_id join works.jobs j on j.org_id = s.org_id and j.job_id = s.job_id where d.org_id = $1 and d.day between $2 and $3 and j.status <> 'cancelled' and ($4::text is null or exists (select 1 from works.project_members m where m.org_id = j.org_id and m.project_id = j.project_id and m.person_id = $4)) order by d.day, s.worker_id, s.seat_id";
