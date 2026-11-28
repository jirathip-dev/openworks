include!(concat!(env!("OUT_DIR"), "/collected.rs"));

mod schema;
mod trail;

pub use schema::*;
pub use trail::*;
