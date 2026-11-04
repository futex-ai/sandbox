//! Private attempt bootstrap, source verification, and trusted control actions.

use std::{sync::Arc, time::Duration};

use base64::{Engine, engine::general_purpose::STANDARD};
use tokio::time::Instant;
use uuid::Uuid;

use crate::{
    error::{Error, Result},
    runtime::{
        api::{BoatRuntimeApi, HelperOutput},
        client::encode,
    },
};

use super::manifest::{Bundle, Manifest, digest};

const HOME: &str = "/home/user";
const STATE: &str = "/var/lib/sandbox-boat";
const BOOTSTRAP: &str = include_str!("helpers/bootstrap.py");
const LOADER: &str = include_str!("helpers/loader.py");

/// Immutable attempt identity retained across all cleanup actions.
#[derive(Clone)]
pub(crate) struct Attempt {
    pub(crate) api: Arc<dyn BoatRuntimeApi>,
    pub(crate) sandbox: String,
    pub(crate) id: String,
    pub(crate) digest: String,
    pub(crate) deadline: Instant,
}

impl Attempt {
    pub(crate) async fn prepare(
        api: Arc<dyn BoatRuntimeApi>,
        sandbox: String,
        workload: String,
        manifest: Manifest,
        deadline: Instant,
    ) -> Result<Self> {
        let id = Uuid::new_v4().simple().to_string();
        let bundle = encode(&Bundle::new(id.clone(), workload.clone(), manifest))?;
        let attempt = Self {
            api,
            sandbox,
            id,
            digest: digest(&bundle),
            deadline,
        };
        let command = python(
            BOOTSTRAP,
            &[HOME, STATE, "/workspace", "user", &workload, &attempt.id],
        );
        let mut preparation = PreparationGuard(Some(attempt.clone()));
        let initialized = attempt
            .api
            .helper(&attempt.sandbox, command, attempt.remaining()?)
            .await?;
        if !initialized.normal || initialized.exit_code != Some(0) || !initialized.stdout.is_empty()
        {
            return Err(Error::InvalidResponse);
        }
        if let Err(error) = attempt
            .api
            .upload(
                &attempt.sandbox,
                attempt.path("bundle.json"),
                bundle,
                attempt.remaining()?,
            )
            .await
        {
            let _ = attempt.bootstrap_cleanup().await;
            preparation.0 = None;
            return Err(error);
        }
        preparation.0 = None;
        Ok(attempt)
    }

    pub(crate) fn path(&self, name: &str) -> String {
        format!("{HOME}/.sandbox-boat/{}/{name}", self.id)
    }

    pub(crate) fn remaining(&self) -> Result<Duration> {
        let remaining = self.deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            Err(Error::Unavailable)
        } else {
            Ok(remaining)
        }
    }

    pub(crate) fn command(&self, action: &str) -> String {
        python(
            LOADER,
            &[HOME, STATE, "user", &self.id, &self.digest, action],
        )
    }

    pub(crate) async fn run(&self, action: &str) -> Result<HelperOutput> {
        self.api
            .helper(
                &self.sandbox,
                self.command(action),
                self.remaining()?.min(Duration::from_secs(300)),
            )
            .await
    }

    pub(crate) async fn cleanup(&self, action: &str) -> Result<HelperOutput> {
        let command = self.command(action);
        let future = self
            .api
            .helper(&self.sandbox, command, Duration::from_secs(2));
        match tokio::time::timeout(Duration::from_secs(2), future).await {
            Ok(result) => result,
            Err(_) => Err(Error::Unavailable),
        }
    }

    async fn bootstrap_cleanup(&self) -> Result<()> {
        let code = include_str!("helpers/abandon.py");
        let command = python(code, &[&self.id]);
        let _ = self
            .api
            .helper(&self.sandbox, command, Duration::from_secs(2))
            .await?;
        Ok(())
    }
}

fn python(source: &str, args: &[&str]) -> String {
    let encoded = STANDARD.encode(source);
    let mut command = format!(
        "/usr/bin/sudo -n /usr/bin/env -i /usr/bin/python3 -I -S -c 'import base64;exec(base64.b64decode(\"{encoded}\"))'"
    );
    for arg in args {
        command.push(' ');
        command.push_str(arg);
    }
    command
}

/// Unfinished preparation cannot have launched caller code. Its guard attempts
/// bounded abandonment even when the awaiting caller cancels the future.
struct PreparationGuard(Option<Attempt>);

impl Drop for PreparationGuard {
    fn drop(&mut self) {
        if let Some(attempt) = self.0.take()
            && let Ok(runtime) = tokio::runtime::Handle::try_current()
        {
            let _cleanup = runtime.spawn(async move {
                let _ =
                    tokio::time::timeout(Duration::from_secs(2), attempt.bootstrap_cleanup()).await;
            });
        }
    }
}
