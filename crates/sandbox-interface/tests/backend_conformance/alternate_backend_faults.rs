//! Fault injection and cleanup accounting for the alternate conformance backend.

use std::{
    collections::HashSet,
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use sandbox_interface::{ProviderRef, SandboxLifetime};

#[derive(Default)]
pub(super) struct AlternateBackendFaults {
    ambiguous_create: AtomicBool,
    one_shot_creates: AtomicUsize,
    recovery_misses: AtomicUsize,
    fail_snapshot_deletes: AtomicBool,
    fail_terminal_closes: AtomicBool,
    created_sandboxes: Mutex<HashSet<String>>,
    destroy_attempts: Mutex<HashSet<String>>,
    created_snapshots: Mutex<HashSet<String>>,
    delete_attempts: Mutex<HashSet<String>>,
    created_terminals: Mutex<HashSet<String>>,
    close_attempts: Mutex<HashSet<String>>,
}

impl AlternateBackendFaults {
    pub(super) fn make_next_create_ambiguous(&self) {
        self.ambiguous_create.store(true, Ordering::Relaxed);
    }

    pub(super) fn set_recovery_misses(&self, misses: usize) {
        self.recovery_misses.store(misses, Ordering::Relaxed);
    }

    pub(super) fn fail_snapshot_deletes(&self) {
        self.fail_snapshot_deletes.store(true, Ordering::Relaxed);
    }

    pub(super) fn fail_terminal_closes(&self) {
        self.fail_terminal_closes.store(true, Ordering::Relaxed);
    }

    pub(super) fn record_sandbox_create(&self, provider_ref: &ProviderRef) -> bool {
        self.created_sandboxes
            .lock()
            .expect("created sandbox lock")
            .insert(provider_ref.as_str().to_owned());
        self.ambiguous_create.swap(false, Ordering::Relaxed)
    }

    pub(super) fn record_sandbox_lifetime(&self, lifetime: SandboxLifetime) {
        if matches!(lifetime, SandboxLifetime::OneShot { .. }) {
            self.one_shot_creates.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub(super) fn one_shot_create_was_exercised(&self) -> bool {
        self.one_shot_creates.load(Ordering::Relaxed) > 0
    }

    pub(super) fn miss_recovery(&self) -> bool {
        self.recovery_misses
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |misses| {
                misses.checked_sub(1)
            })
            .is_ok()
    }

    pub(super) fn record_sandbox_destroy(&self, provider_ref: &ProviderRef) {
        self.destroy_attempts
            .lock()
            .expect("sandbox destroy lock")
            .insert(provider_ref.as_str().to_owned());
    }

    pub(super) fn record_snapshot_create(&self, provider_ref: &ProviderRef) {
        self.created_snapshots
            .lock()
            .expect("created snapshot lock")
            .insert(provider_ref.as_str().to_owned());
    }

    pub(super) fn record_snapshot_delete(&self, provider_ref: &ProviderRef) -> bool {
        self.delete_attempts
            .lock()
            .expect("snapshot delete lock")
            .insert(provider_ref.as_str().to_owned());
        self.fail_snapshot_deletes.load(Ordering::Relaxed)
    }

    pub(super) fn record_terminal_create(&self, provider_ref: &ProviderRef) {
        self.created_terminals
            .lock()
            .expect("created terminal lock")
            .insert(provider_ref.as_str().to_owned());
    }

    pub(super) fn record_terminal_close(&self, provider_ref: &ProviderRef) -> bool {
        self.close_attempts
            .lock()
            .expect("terminal close lock")
            .insert(provider_ref.as_str().to_owned());
        self.fail_terminal_closes.load(Ordering::Relaxed)
    }

    pub(super) fn cleanup_attempted_for_every_resource(&self) -> bool {
        let created_sandboxes = self.created_sandboxes.lock().expect("created sandbox lock");
        let destroy_attempts = self.destroy_attempts.lock().expect("sandbox destroy lock");
        let created_snapshots = self
            .created_snapshots
            .lock()
            .expect("created snapshot lock");
        let delete_attempts = self.delete_attempts.lock().expect("snapshot delete lock");
        let created_terminals = self
            .created_terminals
            .lock()
            .expect("created terminal lock");
        let close_attempts = self.close_attempts.lock().expect("terminal close lock");
        created_sandboxes.is_subset(&destroy_attempts)
            && created_snapshots.is_subset(&delete_attempts)
            && created_terminals.is_subset(&close_attempts)
    }
}
