include!(concat!(env!("OUT_DIR"), "/collected.rs"));

mod router;

pub use router::*;
