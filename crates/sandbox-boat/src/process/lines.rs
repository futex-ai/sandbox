//! Incremental bounded ASCII line storage for both runtime framing layers.

use crate::error::{Error, Result};

#[derive(Default)]
pub(super) struct Lines {
    pending: Vec<u8>,
}

impl Lines {
    pub(super) fn take(&mut self, bytes: &mut &[u8], maximum: usize) -> Result<Option<Vec<u8>>> {
        let end = bytes.iter().position(|b| *b == b'\n');
        let count = end.map_or(bytes.len(), |n| n + 1);
        if count > maximum.saturating_sub(self.pending.len()) {
            return Err(Error::InvalidResponse);
        }
        self.pending.extend_from_slice(&bytes[..count]);
        *bytes = &bytes[count..];
        if end.is_none() {
            return Ok(None);
        }
        self.pending.pop();
        Ok(Some(std::mem::take(&mut self.pending)))
    }
    pub(super) fn empty(&self) -> bool {
        self.pending.is_empty()
    }
}
