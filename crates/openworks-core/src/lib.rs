include!(concat!(env!("OUT_DIR"), "/collected.rs"));

#[cfg(doctest)]
#[doc = include_str!("../tests/undeclared.md")]
struct Undeclared;
