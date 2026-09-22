//! Transfer-path validation helper command tests.

use base64::{Engine as _, engine::general_purpose::STANDARD};

use super::{decode_validation, validation_command};

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
