//! Boat's documented exec, attach, and private file-staging boundaries.

pub mod api;
mod capability;
pub mod client;
pub mod configured;
mod file_path;

#[cfg(test)]
#[path = "_tests_/file_acknowledgment_tests.rs"]
mod file_acknowledgment_tests;
