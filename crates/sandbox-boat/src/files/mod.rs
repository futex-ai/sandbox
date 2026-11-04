//! Confined regular-file transfer independent of provider native file safety.

pub(crate) mod read;
mod validation;
pub(crate) mod write;

#[cfg(test)]
#[path = "_tests_/transfer_bound_tests.rs"]
mod transfer_bound_tests;
