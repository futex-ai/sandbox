//! Trusted Python helper command construction.

pub(crate) const EXECUTABLE: &str = "/usr/bin/python3";

pub(crate) fn command_args(
    script: &str,
    arguments: impl IntoIterator<Item = String>,
) -> Vec<String> {
    let mut args = vec![
        "-I".to_owned(),
        "-S".to_owned(),
        "-c".to_owned(),
        script.to_owned(),
    ];
    args.extend(arguments);
    args
}

#[cfg(test)]
#[path = "_tests_/trusted_python_tests.rs"]
mod trusted_python_tests;
