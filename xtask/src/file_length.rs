//! Rust file-length audit for repository source files.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

const MAX_LINES: usize = 300;
const ROOTS: &[&str] = &["crates", "xtask"];

pub(crate) fn run(root: &Path, _all: bool) -> Result<()> {
    let files = rust_files(root)?;
    let mut violations = Vec::new();
    for path in files {
        let contents = read_file(&path)?;
        let line_count = contents.lines().count();
        if line_count > MAX_LINES {
            violations.push(format!(
                "[xtask/rust_file_length_lint] {} has {line_count} lines; allowed max is {MAX_LINES}",
                display_path(root, &path)
            ));
        }
    }
    if violations.is_empty() {
        Ok(())
    } else {
        Err(Error::FileLengthViolations {
            count: violations.len(),
            details: violations.join("\n"),
        })
    }
}

fn rust_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for relative_root in ROOTS {
        let path = root.join(relative_root);
        if path.exists() {
            collect_rust_files(&path, &mut files)?;
        }
    }
    files.sort();
    Ok(files)
}

fn collect_rust_files(path: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    let entries = fs::read_dir(path).map_err(|source| Error::ReadDir {
        path: path.to_path_buf(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| Error::ReadDir {
            path: path.to_path_buf(),
            source,
        })?;
        let entry_path = entry.path();
        if entry_path.is_dir() {
            collect_rust_files(&entry_path, files)?;
        } else if entry_path
            .extension()
            .is_some_and(|extension| extension == "rs")
        {
            files.push(entry_path);
        }
    }
    Ok(())
}

fn read_file(path: &Path) -> Result<String> {
    fs::read_to_string(path).map_err(|source| Error::ReadFile {
        path: path.to_path_buf(),
        source,
    })
}

fn display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

#[cfg(test)]
#[path = "_tests_/file_length_tests.rs"]
mod file_length_tests;
