//! Resolve pre-release state or decoded child identity without waiting for delivery.

use std::time::Duration;

use sandbox_interface::ProcessStreamOutcome;

use crate::runtime::configured::BoatTrustedRuntime;

use super::reader::{Reader, Terminal};

impl Reader {
    pub(super) async fn run(mut self) -> Terminal {
        let outcome = self.read().await;
        if self.launch.armed() {
            self.launch.cleanup().await;
            if !self.consumer.is_closed() && !self.observed.borrow().ended {
                self.stop_outer().await;
            }
            return Terminal {
                outcome,
                prefix: self.prefix.take(),
            };
        }
        let observation = *self.observed.borrow();
        if self.consumer.is_closed() && observation.pid.is_none() {
            return Terminal {
                outcome,
                prefix: self.prefix.take(),
            };
        }
        let resolved;
        if !observation.ended {
            resolved = self
                .attempt
                .cleanup("terminate")
                .await
                .as_ref()
                .is_ok_and(BoatTrustedRuntime::helper_succeeded);
            self.stop_outer().await;
        } else {
            resolved = self
                .attempt
                .cleanup("ack")
                .await
                .as_ref()
                .is_ok_and(BoatTrustedRuntime::helper_succeeded);
        }
        if outcome == ProcessStreamOutcome::Completed || resolved {
            let _ = self.attempt.cleanup("clean").await;
        }
        Terminal {
            outcome,
            prefix: self.prefix.take(),
        }
    }

    async fn stop_outer(&self) {
        if let Some(outer) = self.outer {
            let api = self.attempt.api.clone();
            let stop = api.stop(
                &self.attempt.sandbox,
                outer.process_id,
                Duration::from_secs(2),
            );
            let _ = tokio::time::timeout(Duration::from_secs(2), stop).await;
        }
    }
}
