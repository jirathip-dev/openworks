use openworks_core::{Fail, Id, Tx};

mod keys;

pub(crate) struct Job {
    pub(crate) queries: &'static [&'static str],
    pub(crate) run: fn(&Id, &mut dyn Tx, i64) -> Result<(), Fail>,
}

pub(crate) const JOBS: &[Job] = &[keys::PRUNE];
