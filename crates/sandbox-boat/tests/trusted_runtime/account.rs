//! Existing non-login workload identity validated by the production bootstrap.

use std::path::Path;

use serde::Deserialize;

use super::fixture::root_python;

pub const BOOTSTRAP: &str = include_str!("../../src/trusted_runtime/helpers/bootstrap.py");

#[derive(Deserialize)]
pub struct Account {
    pub name: String,
    pub uid: u32,
}

pub fn select(home: &Path, state: &Path, workspace: &Path, controller: &str) -> Account {
    let output = root_python(include_str!("account.py"))
        .arg(BOOTSTRAP)
        .args([home.as_os_str(), state.as_os_str(), workspace.as_os_str()])
        .arg(controller)
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(0),
        "account selection: {:?}",
        output.stderr
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
