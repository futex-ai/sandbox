//! Isolated trusted helpers and private per-operation staging.

pub(crate) mod attempt;
pub(crate) mod budget;
pub(crate) mod manifest;

#[cfg(test)]
#[path = "_tests_/preparation_support.rs"]
mod preparation_support;
#[cfg(test)]
#[path = "_tests_/preparation_tests.rs"]
mod preparation_tests;
