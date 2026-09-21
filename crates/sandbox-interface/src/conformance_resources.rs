//! Resource tracking, paced creation recovery, and unconditional conformance cleanup.

use std::time::Duration;

use async_trait::async_trait;

use crate::{
    BackendCreateSandboxRequest, BackendCreateSnapshotRequest, BackendSandbox, BackendSnapshot,
    BackendSnapshotCreateOutcome, BackendSnapshotRecovery, Error, ProviderRef, Result,
    SandboxBackend,
};

const RECOVERY_INTERVAL: Duration = Duration::from_secs(1);
const RECOVERY_WAITS: usize = 60;

/// Delay boundary used to keep paced recovery tests deterministic.
#[unimock::unimock(api = RecoverySleeperMock)]
#[async_trait]
pub(crate) trait RecoverySleeper: Send + Sync {
    /// Waits before the next provider recovery poll.
    async fn sleep(&self, duration: Duration);
}

/// Tokio-backed delay used by the public conformance flow.
pub(crate) struct TokioRecoverySleeper;

#[async_trait]
impl RecoverySleeper for TokioRecoverySleeper {
    async fn sleep(&self, duration: Duration) {
        tokio::time::sleep(duration).await;
    }
}

/// Provider resources owned by one conformance run.
pub(crate) struct ConformanceResources<'a> {
    backend: &'a dyn SandboxBackend,
    sleeper: &'a dyn RecoverySleeper,
    sandboxes: Vec<ProviderRef>,
    pending_sandboxes: Vec<BackendCreateSandboxRequest>,
    snapshots: Vec<ProviderRef>,
    pending_snapshots: Vec<BackendCreateSnapshotRequest>,
    terminals: Vec<(ProviderRef, ProviderRef)>,
}

impl<'a> ConformanceResources<'a> {
    /// Starts an empty resource ledger for one backend.
    pub(crate) fn new(backend: &'a dyn SandboxBackend, sleeper: &'a dyn RecoverySleeper) -> Self {
        Self {
            backend,
            sleeper,
            sandboxes: Vec::new(),
            pending_sandboxes: Vec::new(),
            snapshots: Vec::new(),
            pending_snapshots: Vec::new(),
            terminals: Vec::new(),
        }
    }

    /// Dispatches one sandbox create, then proves its correlated identity.
    pub(crate) async fn create_sandbox(
        &mut self,
        request: BackendCreateSandboxRequest,
    ) -> Result<BackendSandbox> {
        self.pending_sandboxes.push(request.clone());
        let (created, create_error) = match self.backend.create_sandbox(request.clone()).await {
            Ok(sandbox) => {
                self.track_sandbox(&sandbox.provider_ref);
                (Some(sandbox), None)
            }
            Err(error) => (None, Some(error)),
        };
        let mut waits_remaining = RECOVERY_WAITS;
        loop {
            match self.backend.recover_sandbox_create(request.clone()).await? {
                Some(recovered) => {
                    self.track_sandbox(&recovered.provider_ref);
                    self.pending_sandboxes.retain(|pending| pending != &request);
                    if created
                        .as_ref()
                        .is_some_and(|sandbox| sandbox.provider_ref != recovered.provider_ref)
                    {
                        return Err(Error::internal_message(
                            "backend sandbox recovery changed provider identity",
                        ));
                    }
                    return Ok(recovered);
                }
                None if waits_remaining == 0 => {
                    return match create_error {
                        Some(error) => Err(error),
                        None => Err(Error::internal_message(
                            "backend did not recover a created sandbox",
                        )),
                    };
                }
                None => {
                    waits_remaining -= 1;
                    self.sleeper.sleep(RECOVERY_INTERVAL).await;
                }
            }
        }
    }

    /// Dispatches one snapshot create, then proves its correlated identity.
    pub(crate) async fn create_snapshot(
        &mut self,
        request: BackendCreateSnapshotRequest,
    ) -> Result<BackendSnapshot> {
        self.pending_snapshots.push(request.clone());
        let (created, create_error) = match self.backend.create_snapshot(request.clone()).await {
            Ok(BackendSnapshotCreateOutcome::Created(snapshot)) => {
                self.track_snapshot(&snapshot.provider_ref);
                (Some(snapshot), None)
            }
            Ok(BackendSnapshotCreateOutcome::InProgress)
            | Ok(BackendSnapshotCreateOutcome::DeliveryAmbiguous) => (None, None),
            Err(error) => (None, Some(error)),
        };
        let mut waits_remaining = RECOVERY_WAITS;
        loop {
            match self
                .backend
                .recover_snapshot_create(request.clone())
                .await?
            {
                BackendSnapshotRecovery::Recovered(recovered) => {
                    self.track_snapshot(&recovered.provider_ref);
                    self.pending_snapshots.retain(|pending| pending != &request);
                    if created
                        .as_ref()
                        .is_some_and(|snapshot| snapshot.provider_ref != recovered.provider_ref)
                    {
                        return Err(Error::internal_message(
                            "backend snapshot recovery changed provider identity",
                        ));
                    }
                    return Ok(recovered);
                }
                BackendSnapshotRecovery::InProgress if waits_remaining == 0 => {
                    return match create_error {
                        Some(error) => Err(error),
                        None => Err(snapshot_reconciliation_required()),
                    };
                }
                BackendSnapshotRecovery::InProgress => {
                    waits_remaining -= 1;
                    self.sleeper.sleep(RECOVERY_INTERVAL).await;
                }
                BackendSnapshotRecovery::ReconciliationRequired => {
                    return Err(snapshot_reconciliation_required());
                }
            }
        }
    }

    /// Records a terminal before later conformance operations can fail.
    pub(crate) fn track_terminal(&mut self, sandbox: &ProviderRef, terminal: &ProviderRef) {
        let tracked = (sandbox.clone(), terminal.clone());
        if !self.terminals.contains(&tracked) {
            self.terminals.push(tracked);
        }
    }

    /// Attempts every cleanup action and returns only the first cleanup error.
    pub(crate) async fn cleanup(mut self) -> Result<()> {
        let mut first_error = None;
        for (sandbox, terminal) in self.terminals.iter().rev() {
            if let Err(error) = self
                .backend
                .close_terminal(sandbox.clone(), terminal.clone())
                .await
                && !matches!(error, Error::NotFound { .. })
            {
                first_error.get_or_insert(error);
            }
        }
        self.recover_pending_snapshots(&mut first_error).await;
        for snapshot in self.snapshots.iter().rev() {
            if let Err(error) = self.backend.delete_snapshot(snapshot.clone()).await
                && !matches!(error, Error::NotFound { .. })
            {
                first_error.get_or_insert(error);
            }
        }
        self.recover_pending_sandboxes(&mut first_error).await;
        for sandbox in self.sandboxes.iter().rev() {
            if let Err(error) = self.backend.destroy_sandbox(sandbox.clone()).await
                && !matches!(error, Error::NotFound { .. })
            {
                first_error.get_or_insert(error);
            }
        }
        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    async fn recover_pending_snapshots(&mut self, first_error: &mut Option<Error>) {
        for request in std::mem::take(&mut self.pending_snapshots) {
            match self.backend.recover_snapshot_create(request).await {
                Ok(BackendSnapshotRecovery::Recovered(snapshot)) => {
                    self.track_snapshot(&snapshot.provider_ref);
                }
                Ok(BackendSnapshotRecovery::InProgress)
                | Ok(BackendSnapshotRecovery::ReconciliationRequired) => {
                    first_error.get_or_insert_with(snapshot_reconciliation_required);
                }
                Err(error) => {
                    first_error.get_or_insert(error);
                }
            }
        }
    }

    async fn recover_pending_sandboxes(&mut self, first_error: &mut Option<Error>) {
        for request in std::mem::take(&mut self.pending_sandboxes) {
            match self.backend.recover_sandbox_create(request).await {
                Ok(Some(sandbox)) => self.track_sandbox(&sandbox.provider_ref),
                Ok(None) => {
                    first_error.get_or_insert_with(|| {
                        Error::internal_message("backend sandbox cleanup recovery stayed pending")
                    });
                }
                Err(error) => {
                    first_error.get_or_insert(error);
                }
            }
        }
    }

    fn track_sandbox(&mut self, provider_ref: &ProviderRef) {
        if !self.sandboxes.contains(provider_ref) {
            self.sandboxes.push(provider_ref.clone());
        }
    }

    fn track_snapshot(&mut self, provider_ref: &ProviderRef) {
        if !self.snapshots.contains(provider_ref) {
            self.snapshots.push(provider_ref.clone());
        }
    }
}

/// Preserves an operation error while still surfacing cleanup failure on success.
pub(crate) fn finish<T>(outcome: Result<T>, cleanup: Result<()>) -> Result<T> {
    match outcome {
        Err(error) => Err(error),
        Ok(value) => {
            cleanup?;
            Ok(value)
        }
    }
}

fn snapshot_reconciliation_required() -> Error {
    Error::SnapshotReconciliationRequired {
        retained_sandbox: None,
    }
}
