//! Resource tracking, paced creation recovery, and unconditional conformance cleanup.

use std::time::Duration;

use async_trait::async_trait;

use crate::{
    BackendCreateSandboxRequest, BackendCreateSnapshotRequest, BackendSandbox, BackendSnapshot,
    BackendSnapshotCreateOutcome, BackendSnapshotRecovery, Error, ProviderRef, Result,
    SandboxLifecycle, SandboxSnapshots, SandboxTerminals, SnapshotState,
};

#[cfg(test)]
#[path = "_tests_/conformance_start_refusal_tests.rs"]
mod conformance_start_refusal_tests;

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
    sleeper: &'a dyn RecoverySleeper,
    sandboxes: Vec<(&'a dyn SandboxLifecycle, ProviderRef)>,
    pending_sandboxes: Vec<(&'a dyn SandboxLifecycle, BackendCreateSandboxRequest)>,
    snapshots: Vec<(&'a dyn SandboxSnapshots, ProviderRef)>,
    pending_snapshots: Vec<(&'a dyn SandboxSnapshots, BackendCreateSnapshotRequest)>,
    terminals: Vec<(&'a dyn SandboxTerminals, ProviderRef, ProviderRef)>,
}

impl<'a> ConformanceResources<'a> {
    /// Starts an empty ledger that records each resource's exact capability.
    pub(crate) fn new(sleeper: &'a dyn RecoverySleeper) -> Self {
        Self {
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
        lifecycle: &'a dyn SandboxLifecycle,
        request: BackendCreateSandboxRequest,
    ) -> Result<BackendSandbox> {
        self.pending_sandboxes.push((lifecycle, request.clone()));
        let (created, create_error) = match lifecycle.create_sandbox(request.clone()).await {
            Ok(sandbox) => {
                self.track_sandbox(lifecycle, &sandbox.provider_ref);
                (Some(sandbox), None)
            }
            Err(
                error
                @ (Error::SandboxStartRefused { .. } | Error::UnsupportedSizeSelection { .. }),
            ) => {
                self.pending_sandboxes
                    .retain(|(_, pending)| pending != &request);
                return Err(error);
            }
            Err(error) => (None, Some(error)),
        };
        let mut waits_remaining = RECOVERY_WAITS;
        loop {
            match lifecycle.recover_sandbox_create(request.clone()).await? {
                Some(recovered) => {
                    self.track_sandbox(lifecycle, &recovered.provider_ref);
                    self.pending_sandboxes
                        .retain(|(_, pending)| pending != &request);
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
        snapshots: &'a dyn SandboxSnapshots,
        request: BackendCreateSnapshotRequest,
    ) -> Result<BackendSnapshot> {
        self.pending_snapshots.push((snapshots, request.clone()));
        let (created, create_error) = match snapshots.create_snapshot(request.clone()).await {
            Ok(BackendSnapshotCreateOutcome::Created(snapshot)) => {
                self.track_snapshot(snapshots, &snapshot.provider_ref);
                (Some(snapshot), None)
            }
            Ok(BackendSnapshotCreateOutcome::InProgress)
            | Ok(BackendSnapshotCreateOutcome::DeliveryAmbiguous) => (None, None),
            Err(error) => (None, Some(error)),
        };
        let mut waits_remaining = RECOVERY_WAITS;
        loop {
            match snapshots.recover_snapshot_create(request.clone()).await? {
                BackendSnapshotRecovery::Recovered(recovered) => {
                    self.track_snapshot(snapshots, &recovered.provider_ref);
                    self.pending_snapshots
                        .retain(|(_, pending)| pending != &request);
                    if created
                        .as_ref()
                        .is_some_and(|snapshot| snapshot.provider_ref != recovered.provider_ref)
                    {
                        return Err(Error::internal_message(
                            "backend snapshot recovery changed provider identity",
                        ));
                    }
                    if recovered.state != SnapshotState::Ready {
                        return Err(Error::SnapshotStateConflict {
                            state: recovered.state,
                        });
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
    pub(crate) fn track_terminal(
        &mut self,
        capability: &'a dyn SandboxTerminals,
        sandbox: &ProviderRef,
        terminal: &ProviderRef,
    ) {
        if !self
            .terminals
            .iter()
            .any(|(_, tracked_sandbox, tracked_terminal)| {
                tracked_sandbox == sandbox && tracked_terminal == terminal
            })
        {
            self.terminals
                .push((capability, sandbox.clone(), terminal.clone()));
        }
    }

    /// Attempts every cleanup action and returns only the first cleanup error.
    pub(crate) async fn cleanup(mut self) -> Result<()> {
        let mut first_error = None;
        for (terminals, sandbox, terminal) in self.terminals.iter().rev() {
            if let Err(error) = terminals
                .close_terminal(sandbox.clone(), terminal.clone())
                .await
                && !matches!(error, Error::NotFound { .. })
            {
                first_error.get_or_insert(error);
            }
        }
        self.recover_pending_snapshots(&mut first_error).await;
        for (snapshots, snapshot) in self.snapshots.iter().rev() {
            if let Err(error) = snapshots.delete_snapshot(snapshot.clone()).await
                && !matches!(error, Error::NotFound { .. })
            {
                first_error.get_or_insert(error);
            }
        }
        self.recover_pending_sandboxes(&mut first_error).await;
        for (lifecycle, sandbox) in self.sandboxes.iter().rev() {
            if let Err(error) = lifecycle.destroy_sandbox(sandbox.clone()).await
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
        for (snapshots, request) in std::mem::take(&mut self.pending_snapshots) {
            match snapshots.recover_snapshot_create(request).await {
                Ok(BackendSnapshotRecovery::Recovered(snapshot)) => {
                    self.track_snapshot(snapshots, &snapshot.provider_ref);
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
        for (lifecycle, request) in std::mem::take(&mut self.pending_sandboxes) {
            match lifecycle.recover_sandbox_create(request).await {
                Ok(Some(sandbox)) => self.track_sandbox(lifecycle, &sandbox.provider_ref),
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

    fn track_sandbox(&mut self, capability: &'a dyn SandboxLifecycle, provider_ref: &ProviderRef) {
        if !self
            .sandboxes
            .iter()
            .any(|(_, tracked)| tracked == provider_ref)
        {
            self.sandboxes.push((capability, provider_ref.clone()));
        }
    }

    fn track_snapshot(&mut self, capability: &'a dyn SandboxSnapshots, provider_ref: &ProviderRef) {
        if !self
            .snapshots
            .iter()
            .any(|(_, tracked)| tracked == provider_ref)
        {
            self.snapshots.push((capability, provider_ref.clone()));
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
