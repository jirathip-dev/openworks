use openworks_core::{Fail, Id, Tx, Value};
use openworks_db::idempotency::PRUNE as PRUNE_SQL;

use super::Job;

const KEPT_FOR: i64 = 24 * 60 * 60 * 1000;

pub(super) const PRUNE: Job = Job {
    queries: &[PRUNE_SQL],
    run: prune,
};

fn prune(org: &Id, tx: &mut dyn Tx, now: i64) -> Result<(), Fail> {
    tx.run(
        PRUNE_SQL,
        &[Value::Text(org.0.clone()), Value::Int(now.saturating_sub(KEPT_FOR))],
    )?;
    Ok(())
}
