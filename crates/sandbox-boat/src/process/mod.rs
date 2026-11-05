//! Bounded workload execution over Boat detached commands and live attachment.

mod cleanup;
pub(crate) mod collected;
pub(crate) mod execution;
mod framing;
mod launch;
mod lines;
mod reader;
mod settings;
mod stream;
mod wait;

#[cfg(test)]
#[path = "_tests_/timing_support.rs"]
mod timing_support;
#[cfg(test)]
#[path = "_tests_/timing_tests.rs"]
mod timing_tests;

#[cfg(test)]
#[path = "_tests_/drop_tests.rs"]
mod drop_tests;

#[cfg(test)]
#[path = "_tests_/gated_api.rs"]
mod gated_api;
#[cfg(test)]
#[path = "_tests_/startup_support.rs"]
mod startup_support;
#[cfg(test)]
#[path = "_tests_/startup_tests.rs"]
mod startup_tests;
