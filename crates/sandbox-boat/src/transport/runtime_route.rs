//! Runtime routes restricted to the configured HTTPS authority.

use crate::error::{Error, Result};

use super::request::valid_sandbox_id;

/// Typed command and protected native-staging routes.
pub enum RuntimeRoute {
    /// One initial command dispatch.
    Commands(String),
    /// One detached command termination.
    Stop { sandbox: String, process: u64 },
    /// Offset-resuming live command output.
    Attach {
        sandbox: String,
        process: u64,
        stdout: u64,
        stderr: u64,
    },
    /// Protected staging upload.
    Upload { sandbox: String },
    /// Protected staging download.
    Download { sandbox: String, path: String },
}

impl RuntimeRoute {
    pub(crate) fn path(&self) -> Result<(reqwest::Method, String)> {
        let (sandbox, method, suffix) = match self {
            Self::Commands(id) => (id, reqwest::Method::POST, "/commands".to_owned()),
            Self::Stop { sandbox, process } if *process > 0 => (
                sandbox,
                reqwest::Method::DELETE,
                format!("/commands/{process}"),
            ),
            Self::Attach {
                sandbox,
                process,
                stdout,
                stderr,
            } if *process > 0 => (
                sandbox,
                reqwest::Method::GET,
                format!("/commands/{process}/stream?stdoutOffset={stdout}&stderrOffset={stderr}"),
            ),
            Self::Upload { sandbox } => (sandbox, reqwest::Method::PUT, "/files".to_owned()),
            Self::Download { sandbox, path } if valid_stage_path(path) => {
                let mut query = url::form_urlencoded::Serializer::new(String::new());
                query
                    .append_pair("path", path)
                    .append_pair("encoding", "base64");
                (
                    sandbox,
                    reqwest::Method::GET,
                    format!("/files?{}", query.finish()),
                )
            }
            _ => return Err(Error::InvalidRequest),
        };
        if !valid_sandbox_id(sandbox) {
            return Err(Error::InvalidRequest);
        }
        Ok((method, format!("/sandboxes/{sandbox}{suffix}")))
    }
}

pub(crate) fn valid_stage_path(path: &str) -> bool {
    let Some(rest) = path.strip_prefix("/home/user/.sandbox-boat/") else {
        return false;
    };
    let Some((attempt, name)) = rest.split_once('/') else {
        return false;
    };
    attempt.len() == 32
        && attempt.bytes().all(|b| b.is_ascii_hexdigit())
        && !name.is_empty()
        && name.len() < 80
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.'))
}
