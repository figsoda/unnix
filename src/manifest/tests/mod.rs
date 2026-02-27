#![cfg(test)]

use insta::assert_debug_snapshot;

use super::Manifest;

macro_rules! manifest {
    ($path:literal) => {
        Manifest::parse(include_str!($path)).unwrap()
    };
}

#[test]
fn basic() {
    assert_debug_snapshot!(manifest!("basic.kdl"));
}

#[test]
fn cache_no_default() {
    assert_debug_snapshot!(manifest!("cache-no-default.kdl"));
}

#[test]
fn hydra() {
    assert_debug_snapshot!(manifest!("hydra.kdl"));
}
