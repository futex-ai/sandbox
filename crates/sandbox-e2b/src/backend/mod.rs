//! Provider-neutral E2B backend composition and dispatch.

pub(crate) mod configured;
mod files;
mod image_command_diagnostic;
mod image_realization;
mod image_snapshot;
mod mapping;
mod port_ingress;
mod process_run;
mod read_only_exec;
mod sandboxes;
mod screen_resize;
mod screen_stack;
mod snapshots;
mod terminal_identity;
mod terminal_output;
mod terminals;

#[cfg(test)]
#[path = "_tests_/backend_tests.rs"]
mod backend_tests;

#[cfg(test)]
#[path = "_tests_/sandbox_create_tests.rs"]
mod sandbox_create_tests;

#[cfg(test)]
#[path = "_tests_/backend_conformance_tests.rs"]
mod backend_conformance_tests;

#[cfg(test)]
#[path = "_tests_/image_snapshot_tests.rs"]
mod image_snapshot_tests;

#[cfg(test)]
#[path = "_tests_/port_ingress_tests.rs"]
mod port_ingress_tests;

#[cfg(test)]
#[path = "_tests_/read_only_exec_tests.rs"]
mod read_only_exec_tests;
#[cfg(test)]
#[path = "_tests_/screen_stack/mod.rs"]
mod screen_stack_tests;

#[cfg(test)]
#[path = "_tests_/image_realization_tests.rs"]
mod image_realization_tests;

#[cfg(test)]
#[path = "_tests_/image_recovery_tests.rs"]
mod image_recovery_tests;

#[cfg(test)]
#[path = "_tests_/image_command_diagnostic_tests.rs"]
mod image_command_diagnostic_tests;

#[cfg(test)]
#[path = "_tests_/managed_sandbox_tests.rs"]
mod managed_sandbox_tests;

#[cfg(test)]
#[path = "_tests_/snapshot_delete_tests.rs"]
mod snapshot_delete_tests;

#[cfg(test)]
#[path = "_tests_/terminal_recovery_tests.rs"]
mod terminal_recovery_tests;

#[cfg(test)]
#[path = "_tests_/terminal_identity_tests.rs"]
mod terminal_identity_tests;

#[cfg(test)]
#[path = "_tests_/terminal_wait_tests.rs"]
mod terminal_wait_tests;

#[cfg(test)]
#[path = "_tests_/terminal_deadline_tests.rs"]
mod terminal_deadline_tests;
