//! Call-local E2B envd routing, credentials, and process identity.

use std::fmt;

/// Call-local E2B envd connection details.
#[derive(Clone, Eq, PartialEq)]
pub struct ProcessConnection {
    pub(crate) sandbox_id: String,
    pub(crate) sandbox_domain: String,
    pub(crate) access_token: String,
    user: Option<String>,
}

impl ProcessConnection {
    /// Creates call-local routing details that are redacted from debug output.
    pub fn new(sandbox_id: String, sandbox_domain: String, access_token: String) -> Self {
        Self {
            sandbox_id,
            sandbox_domain,
            access_token,
            user: None,
        }
    }

    pub(crate) fn with_user(mut self, user: impl Into<String>) -> Self {
        self.user = Some(user.into());
        self
    }

    pub(crate) fn user(&self) -> Option<&str> {
        self.user.as_deref()
    }

    /// Borrows the opaque provider sandbox identifier.
    #[must_use]
    pub fn sandbox_id(&self) -> &str {
        &self.sandbox_id
    }

    /// Borrows the configured provider routing domain.
    #[must_use]
    pub fn sandbox_domain(&self) -> &str {
        &self.sandbox_domain
    }

    /// Borrows the call-local envd token for an outbound request.
    #[must_use]
    pub fn access_token(&self) -> &str {
        &self.access_token
    }
}

impl fmt::Debug for ProcessConnection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProcessConnection")
            .field("sandbox_id", &"[redacted]")
            .field("sandbox_domain", &self.sandbox_domain)
            .field("access_token", &"[redacted]")
            .field("user", &self.user)
            .finish()
    }
}
