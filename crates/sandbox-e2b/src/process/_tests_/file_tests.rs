//! Provider-log helper command tests.

use base64::{Engine as _, engine::general_purpose::STANDARD};

use super::{decode_validation, read_command, validation_command};

#[test]
fn provider_log_reader_does_not_load_shell_profiles() {
    let command = read_command("/tmp/sandbox.log".to_owned(), 0, 1024);

    assert_eq!(command.command, "/bin/sh");
    assert_eq!(command.args[0], "-c");
}

#[test]
fn provider_log_reader_observes_size_after_reading_the_chunk() {
    let command = read_command("/tmp/sandbox.log".to_owned(), 8, 1024);
    let script = &command.args[1];
    let read_position = script.find("dd if=").expect("chunk read command");
    let size_position = script.find("wc -c").expect("post-read size command");

    assert!(read_position < size_position);
}

#[test]
fn validation_checks_every_path_component_for_symlinks() {
    let command = validation_command(
        "/tmp/repository".to_owned(),
        "src/nested/lib.rs".to_owned(),
        false,
    );
    let script = &command.args[1];

    assert!(script.contains("set -f"));
    assert!(script.contains("for s do"));
    assert!(script.contains("test ! -L \"$c\" || sl=1"));
    assert!(script.contains("test \"$sl\" = 1; then cp=$(realpath -m"));
    assert!(script.contains("\"$e\" \"$rg\" \"$sl\" \"$sz\""));
}

#[test]
fn validation_decoder_preserves_the_symlink_flag() {
    let root = STANDARD.encode("/tmp/repository");
    let path = STANDARD.encode("/tmp/repository/src/lib.rs");
    let response = format!("{root}\n{path}\n1\n1\n1\n42");

    let validation = decode_validation(response.as_bytes()).expect("validation response");

    assert!(validation.exists);
    assert!(validation.regular);
    assert!(validation.symlink);
    assert_eq!(validation.size, 42);
}
