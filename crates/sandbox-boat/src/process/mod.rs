//! Bounded workload execution over Boat detached commands and live attachment.

pub(crate) mod collected;
pub(crate) mod execution;
mod framing;
mod lines;
mod reader;
mod settings;
mod stream;

#[cfg(test)]
#[path = "_tests_/timing_support.rs"]
mod timing_support;
#[cfg(test)]
#[path = "_tests_/timing_tests.rs"]
mod timing_tests;

#[cfg(test)]
#[path = "_tests_/drop_tests.rs"]
mod drop_tests;
