//! Redaction-safe debugging for process commands that may carry secrets.

use std::{collections::BTreeMap, fmt};

use super::types::{ProcessCommand, SplitProcessCommand};

impl fmt::Debug for ProcessCommand {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProcessCommand")
            .field("command", &self.command)
            .field("args", &self.args)
            .field("cwd", &self.cwd)
            .field("envs", &RedactedEnvironment(&self.envs))
            .field("output_capture", &self.output_capture)
            .field("timeout", &self.timeout)
            .field("read_only", &self.read_only)
            .finish()
    }
}

impl fmt::Debug for SplitProcessCommand {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SplitProcessCommand")
            .field("command", &self.command)
            .field("args", &self.args)
            .field("cwd", &self.cwd)
            .field("envs", &RedactedEnvironment(&self.envs))
            .field("stdout_limit", &self.stdout_limit)
            .field("stderr_limit", &self.stderr_limit)
            .field("deadline", &self.deadline)
            .finish()
    }
}

struct RedactedEnvironment<'a>(&'a BTreeMap<String, String>);

impl fmt::Debug for RedactedEnvironment<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut map = formatter.debug_map();
        for name in self.0.keys() {
            map.entry(name, &"[REDACTED]");
        }
        map.finish()
    }
}

#[cfg(test)]
#[path = "_tests_/command_debug_tests.rs"]
mod command_debug_tests;
