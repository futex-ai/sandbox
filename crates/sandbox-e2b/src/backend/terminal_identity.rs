//! Durable E2B terminal identity encoding and process-tag verification.

use sandbox_interface::{Error, ProviderRef, ResourceKind, Result, TerminalId};

use crate::process::ProcessInfo;

const PROVIDER_REF_PREFIX: &str = "e2b-pty-v1:";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct TerminalIdentity {
    pid: u32,
    terminal_id: TerminalId,
}

impl TerminalIdentity {
    pub(super) const fn new(pid: u32, terminal_id: TerminalId) -> Self {
        Self { pid, terminal_id }
    }

    pub(super) fn parse(provider_ref: &ProviderRef) -> Result<Self> {
        let Some(encoded) = provider_ref.as_str().strip_prefix(PROVIDER_REF_PREFIX) else {
            return Err(Error::internal_message(
                "E2B terminal provider ref omitted its versioned identity",
            ));
        };
        let Some((pid, terminal_id)) = encoded.split_once(':') else {
            return Err(Error::internal_message(
                "E2B terminal provider ref omitted its consumer handle",
            ));
        };
        let pid = match pid.parse() {
            Ok(pid) => pid,
            Err(source) => return Err(Error::internal_with(source, "parse E2B terminal PID")),
        };
        let terminal_id = match terminal_id.parse() {
            Ok(terminal_id) => terminal_id,
            Err(source) => {
                return Err(Error::internal_with(
                    source,
                    "parse E2B terminal consumer handle",
                ));
            }
        };
        Ok(Self { pid, terminal_id })
    }

    pub(super) fn provider_ref(self) -> ProviderRef {
        ProviderRef::new(format!(
            "{PROVIDER_REF_PREFIX}{}:{}",
            self.pid, self.terminal_id
        ))
    }

    pub(super) const fn pid(self) -> u32 {
        self.pid
    }

    pub(super) const fn terminal_id(self) -> TerminalId {
        self.terminal_id
    }

    pub(super) fn resolve(
        self,
        processes: Vec<ProcessInfo>,
        terminal_tag_prefix: &str,
    ) -> Result<Option<ProcessInfo>> {
        let expected_tag = terminal_tag(terminal_tag_prefix, self.terminal_id);
        if let Some(process) = processes.iter().find(|process| {
            process.pid == self.pid && process.tag.as_deref() == Some(expected_tag.as_str())
        }) {
            return Ok(Some(process.clone()));
        }
        if processes.iter().any(|process| process.pid == self.pid) {
            return Err(Error::NotFound {
                resource: ResourceKind::Terminal,
            });
        }
        Ok(None)
    }
}

pub(super) fn terminal_tag(terminal_tag_prefix: &str, terminal_id: TerminalId) -> String {
    format!("{terminal_tag_prefix}{terminal_id}")
}

pub(super) fn tagged_terminal<'a>(
    process: &'a ProcessInfo,
    terminal_tag_prefix: &str,
) -> Option<&'a str> {
    process.tag.as_deref()?.strip_prefix(terminal_tag_prefix)
}
