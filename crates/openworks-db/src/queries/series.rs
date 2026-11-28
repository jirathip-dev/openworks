/// Takes the next number of a series in a period, locking its counter row until the transaction ends.
pub const NEXT: &str = "insert into works.series (org_id, series_key, period, last) values ($1, $2, $3, 1) on conflict (org_id, series_key, period) do update set last = works.series.last + 1 returning last";
