//! Exact terminal transcript limit coverage.

use std::{
    fs,
    io::Write,
    os::unix::fs::{PermissionsExt, symlink},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use sandbox_interface::{OperationId, TerminalId};
use tempfile::tempdir;

use super::{ListResponseWire, TERMINAL_WRAPPER, decode};

#[test]
fn list_response_rejects_a_zero_pid() {
    let result = decode::<ListResponseWire>(br#"{"processes":[{"pid":0,"tag":"terminal"}]}"#);

    assert!(result.is_err());
}

#[test]
fn terminal_wrapper_enforces_non_aligned_byte_limits_exactly() {
    for limit in [0, 64, 4097] {
        let directory = tempdir().expect("temporary transcript directory");
        let transcript = directory.path().join("terminal.log");
        let mut child = start_wrapper(&transcript, limit);
        let mut input = child.stdin.take().expect("terminal wrapper stdin");
        input
            .write_all(b"printf '%010000d' 0\n")
            .expect("write terminal command");
        let observed_size = wait_for_transcript_size(&transcript, limit);
        input.write_all(b"exit\n").expect("exit terminal shell");

        let status = child.wait().expect("wait for bounded terminal wrapper");

        assert!(status.success(), "terminal wrapper did not exit normally");
        assert_eq!(observed_size, u64::try_from(limit).expect("test limit"));
        assert_eq!(
            std::fs::metadata(&transcript)
                .expect("bounded transcript")
                .len(),
            u64::try_from(limit).expect("test limit")
        );
    }
}

fn wait_for_transcript_size(path: &std::path::Path, expected: usize) -> u64 {
    let deadline = Instant::now() + Duration::from_secs(4);
    let expected = u64::try_from(expected).expect("test limit");
    loop {
        let observed = fs::metadata(path).map_or(0, |metadata| metadata.len());
        if observed >= expected || Instant::now() >= deadline {
            return observed;
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn terminal_wrapper_does_not_follow_an_existing_log_symlink() {
    let directory = tempdir().expect("temporary transcript directory");
    let outside = tempdir().expect("outside transcript directory");
    let transcript = directory.path().join("terminal.log");
    let protected = outside.path().join("protected.log");
    fs::write(&protected, b"protected").expect("protected file");
    symlink(&protected, &transcript).expect("transcript symlink");
    let mut child = start_wrapper(&transcript, 1024 * 1024);
    let mut input = child.stdin.take().expect("terminal wrapper stdin");
    input
        .write_all(b"echo captured\nexit\n")
        .expect("write terminal command");

    let status = child.wait().expect("wait for terminal wrapper");

    assert!(status.success(), "terminal wrapper did not exit normally");
    assert_eq!(fs::read(&protected).expect("protected file"), b"protected");
    assert!(
        !fs::symlink_metadata(&transcript)
            .expect("replacement transcript")
            .file_type()
            .is_symlink()
    );
}

#[test]
fn terminal_wrapper_rejects_a_symlinked_parent_directory() {
    let directory = tempdir().expect("temporary transcript directory");
    let outside = tempdir().expect("outside transcript directory");
    let linked_parent = directory.path().join("linked");
    symlink(outside.path(), &linked_parent).expect("parent symlink");
    let transcript = linked_parent.join("terminal.log");
    let mut child = start_wrapper(&transcript, 1024 * 1024);

    let status = child.wait().expect("wait for terminal wrapper");

    assert!(!status.success(), "symlinked parent should fail closed");
    assert!(!outside.path().join("terminal.log").exists());
}

#[test]
fn terminal_wrapper_exits_normally_below_the_limit() {
    let directory = tempdir().expect("temporary transcript directory");
    let transcript = directory.path().join("terminal.log");
    let mut child = start_wrapper(&transcript, 1024 * 1024);
    let mut input = child.stdin.take().expect("terminal wrapper stdin");
    input
        .write_all(b"printf '\\143\\157\\155\\160\\154\\145\\164\\145\\n'\nexit\n")
        .expect("write terminal command");

    let status = child.wait().expect("wait for terminal wrapper");

    assert!(status.success(), "terminal wrapper did not exit normally");
    let transcript = fs::read(transcript).expect("completed transcript");
    assert!(
        transcript
            .windows(b"complete".len())
            .any(|part| part == b"complete"),
        "terminal wrapper exited before draining the transcript"
    );
    assert!(!transcript.is_empty() && transcript.len() < 1024 * 1024);
}

#[test]
fn terminal_wrapper_persists_a_private_versioned_identity() {
    let directory = tempdir().expect("temporary transcript directory");
    let transcript = directory.path().join("terminal.log");
    let identity = transcript.with_extension("identity.json");
    let mut child = start_wrapper(&transcript, 1024);
    let mut input = child.stdin.take().expect("terminal wrapper stdin");
    input.write_all(b"exit\n").expect("exit terminal shell");

    let status = child.wait().expect("wait for terminal wrapper");
    let metadata = fs::metadata(&identity).expect("durable terminal identity");
    let record: serde_json::Value =
        serde_json::from_slice(&fs::read(identity).expect("read durable terminal identity"))
            .expect("versioned terminal identity JSON");

    assert!(status.success(), "terminal wrapper did not exit normally");
    assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
    assert_eq!(record["schema"], "sandbox-e2b-terminal-identity-v1");
    assert!(record["pid"].as_u64().is_some_and(|pid| pid > 0));
}

#[test]
fn terminal_identity_is_hidden_until_its_contents_are_synced() {
    let directory = tempdir().expect("temporary transcript directory");
    let transcript = directory.path().join("terminal.log");
    let identity = transcript.with_extension("identity.json");
    let marker = directory.path().join("identity-synced.marker");
    let mut child = start_wrapper_paused_after_identity_sync(&transcript, &marker);

    wait_for_path(&marker);
    let published_before_sync_completed = identity.exists();
    fs::remove_file(&marker).expect("release identity publisher");
    let mut input = child.stdin.take().expect("terminal wrapper stdin");
    input.write_all(b"exit\n").expect("exit terminal shell");
    let status = child.wait().expect("wait for terminal wrapper");

    assert!(status.success(), "terminal wrapper did not exit normally");
    assert!(
        !published_before_sync_completed,
        "final identity name became visible before atomic publication"
    );
    assert!(identity.is_file());
}

fn start_wrapper(path: &std::path::Path, limit: usize) -> std::process::Child {
    let wrapper = test_wrapper();
    spawn_wrapper(&wrapper, path, limit, None)
}

fn start_wrapper_paused_after_identity_sync(
    path: &std::path::Path,
    marker: &std::path::Path,
) -> std::process::Child {
    let wrapper = test_wrapper();
    let sync = "    os.fsync(identity)\n";
    let paused = concat!(
        "    os.fsync(identity)\n",
        "    marker = sys.argv[8]\n",
        "    with open(marker, 'x', encoding='utf-8'):\n",
        "        pass\n",
        "    while os.path.exists(marker):\n",
        "        time.sleep(0.01)\n",
    );
    let wrapper = wrapper.replacen(sync, paused, 1);
    assert_ne!(wrapper, test_wrapper(), "identity sync hook changed");
    spawn_wrapper(&wrapper, path, 1024, Some(marker))
}

fn spawn_wrapper(
    wrapper: &str,
    path: &std::path::Path,
    limit: usize,
    marker: Option<&std::path::Path>,
) -> std::process::Child {
    let identity_path = path.with_extension("identity.json");
    let terminal_id = TerminalId::new();
    let parent = path.parent().expect("terminal transcript parent");
    if fs::symlink_metadata(parent)
        .expect("terminal transcript parent metadata")
        .file_type()
        .is_dir()
    {
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
            .expect("private terminal transcript parent");
    }
    let mut command = Command::new("/usr/bin/timeout");
    command
        .args(["5s", "/usr/bin/python3", "-I", "-S", "-c", wrapper])
        .arg(path)
        .arg(identity_path)
        .arg(limit.to_string())
        .arg(current_username())
        .arg(terminal_id.to_string())
        .arg(OperationId::new().to_string())
        .arg(format!("sandbox-terminal-{terminal_id}"));
    if let Some(marker) = marker {
        command.arg(marker);
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("start terminal wrapper")
}

fn wait_for_path(path: &std::path::Path) {
    let deadline = Instant::now() + Duration::from_secs(4);
    while !path.exists() {
        assert!(Instant::now() < deadline, "timed out waiting for {path:?}");
        thread::sleep(Duration::from_millis(10));
    }
}

fn test_wrapper() -> String {
    let production_shell = "os.execv('/bin/bash', ['bash', '-il'])";
    let deterministic_shell = "os.execv('/bin/bash', ['bash', '--noprofile', '--norc', '-i'])";
    let production_drop = r#"    os.initgroups(username, account.pw_gid)
    os.setgid(account.pw_gid)
    os.setuid(account.pw_uid)"#;
    let same_user_check = r#"    if os.geteuid() != account.pw_uid or os.getegid() != account.pw_gid:
        fail()"#;
    let shell_replaced = TERMINAL_WRAPPER.replace(production_shell, deterministic_shell);
    assert_ne!(
        shell_replaced, TERMINAL_WRAPPER,
        "production shell command changed"
    );
    let wrapper = shell_replaced.replace(production_drop, same_user_check);
    assert_ne!(wrapper, shell_replaced, "production privilege drop changed");
    wrapper
}

fn current_username() -> String {
    let output = Command::new("/usr/bin/id")
        .arg("-un")
        .output()
        .expect("resolve current username");
    assert!(output.status.success());
    String::from_utf8(output.stdout)
        .expect("UTF-8 username")
        .trim()
        .to_owned()
}
