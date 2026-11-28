use openworks::Files;
use openworks_core::Fail;

/// A file store that keeps nothing.
pub struct NoFiles;

impl Files for NoFiles {
    fn put(&self, _: &str, _: &[u8]) -> Result<(), Fail> {
        Ok(())
    }
    fn get(&self, _: &str) -> Result<Option<Vec<u8>>, Fail> {
        Ok(None)
    }
}
