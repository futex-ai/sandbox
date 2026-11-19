//! Checked comparisons for Boat's POSIX file acknowledgment paths.

/// Requires the same normalized file inside Boat's documented sandbox home.
pub(super) fn matches(requested: &str, acknowledged: &str) -> bool {
    match (normalize(requested), normalize(acknowledged)) {
        (Some(requested), Some(acknowledged)) => requested == acknowledged,
        _ => false,
    }
}

/// Resolves relative paths lexically without using the host filesystem.
fn normalize(path: &str) -> Option<Vec<&str>> {
    let home = ["home", "user"];
    let mut parts = if path.starts_with('/') {
        Vec::new()
    } else {
        home.to_vec()
    };
    for part in path.split('/') {
        match part {
            ".." => return None,
            "" | "." => {}
            _ => parts.push(part),
        }
    }
    (parts.len() > home.len() && parts.starts_with(&home)).then_some(parts)
}
