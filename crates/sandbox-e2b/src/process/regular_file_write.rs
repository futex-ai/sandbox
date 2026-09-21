//! Atomic descriptor-relative regular-file replacement inside an E2B sandbox.

use std::time::Duration;

use sandbox_interface::{Error, FILE_TRANSFER_MAX_BYTES, Result};
use uuid::Uuid;

use super::{
    connect::ConnectProcessTransport,
    mapping::map_file_result,
    types::{ProcessCommand, ProcessConnection, ProcessOutputCapture},
};

const WRITE_TIMEOUT: Duration = Duration::from_secs(300);

/// One atomic replacement write below a trusted absolute root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessRegularFileWriteRequest {
    /// Absolute directory that contains the destination.
    pub root: String,
    /// Normalized root-relative destination path.
    pub path: String,
    /// Complete replacement contents.
    pub bytes: Vec<u8>,
}

const WRITER: &str = r#"import errno
import os
import stat
import sys

INVALID_ROOT = 45
NOT_REGULAR = 46
TOO_LARGE = 47
INVALID_PATH = 48
FAILED = 49

def stop(code):
    raise SystemExit(code)

def open_below(directory, name, flags, failure):
    try:
        return os.open(name, flags, dir_fd=directory)
    except OSError as error:
        if error.errno in (errno.ELOOP, errno.ENOTDIR):
            stop(NOT_REGULAR)
        stop(failure)

staging = None
temporary = None
directory = None
source = None
destination = None
try:
    root = sys.argv[1]
    path = sys.argv[2]
    staging = sys.argv[3]
    temporary = sys.argv[4]
    expected = int(sys.argv[5])
    transfer_limit = int(sys.argv[6])
    root_parts = [part for part in root.split('/') if part]
    path_parts = path.split('/')
    if root != '/' + '/'.join(root_parts):
        stop(INVALID_ROOT)
    if not path_parts or any(part in ('', '.', '..') for part in path_parts):
        stop(INVALID_PATH)
    if expected < 0 or expected > transfer_limit:
        stop(TOO_LARGE)
    directory_flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
    directory = os.open('/', directory_flags)
    for part in root_parts:
        child = open_below(directory, part, directory_flags, INVALID_ROOT)
        os.close(directory)
        directory = child
    for part in path_parts[:-1]:
        child = open_below(directory, part, directory_flags, INVALID_PATH)
        os.close(directory)
        directory = child
    target = path_parts[-1]
    try:
        target_metadata = os.stat(target, dir_fd=directory, follow_symlinks=False)
    except FileNotFoundError:
        mode = 0o600
    except OSError:
        stop(FAILED)
    else:
        if not stat.S_ISREG(target_metadata.st_mode):
            stop(NOT_REGULAR)
        mode = stat.S_IMODE(target_metadata.st_mode)
    source_flags = os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC
    source = os.open(staging, source_flags)
    source_metadata = os.fstat(source)
    if not stat.S_ISREG(source_metadata.st_mode):
        stop(NOT_REGULAR)
    if source_metadata.st_size != expected:
        stop(FAILED)
    os.unlink(staging)
    staging = None
    destination_flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC
    destination = os.open(temporary, destination_flags, mode, dir_fd=directory)
    os.fchmod(destination, mode)
    remaining = expected
    while remaining:
        chunk = os.read(source, min(remaining, 65536))
        if not chunk:
            stop(FAILED)
        view = memoryview(chunk)
        while view:
            written = os.write(destination, view)
            view = view[written:]
        remaining -= len(chunk)
    if os.read(source, 1):
        stop(FAILED)
    os.fsync(destination)
    os.close(destination)
    destination = None
    os.replace(temporary, target, src_dir_fd=directory, dst_dir_fd=directory)
    temporary = None
except (IndexError, ValueError):
    stop(INVALID_PATH)
except OSError:
    stop(FAILED)
finally:
    if destination is not None:
        os.close(destination)
    if source is not None:
        os.close(source)
    if temporary is not None and directory is not None:
        try:
            os.unlink(temporary, dir_fd=directory)
        except OSError:
            pass
    if directory is not None:
        os.close(directory)
    if staging is not None:
        try:
            os.unlink(staging)
        except OSError:
            pass
"#;

pub(super) async fn write(
    transport: &ConnectProcessTransport,
    connection: ProcessConnection,
    request: ProcessRegularFileWriteRequest,
) -> Result<()> {
    if request.bytes.len() > FILE_TRANSFER_MAX_BYTES {
        return Err(Error::FileTooLarge {
            limit: FILE_TRANSFER_MAX_BYTES,
        });
    }
    let nonce = Uuid::new_v4().simple().to_string();
    let staging_path = format!("/tmp/.sandbox-e2b-stage-{nonce}");
    let temporary_name = format!(".sandbox-e2b-write-{nonce}");
    let expected_size = request.bytes.len();
    map_file_result(
        transport
            .http
            .upload(connection.clone(), staging_path.clone(), request.bytes)
            .await,
        &transport.backend_id,
    )?;
    let output = transport
        .run_helper(
            connection,
            command(
                request.root,
                request.path,
                staging_path,
                temporary_name,
                expected_size,
            ),
            WRITE_TIMEOUT,
        )
        .await?;
    match output.exit_code {
        Some(0) => Ok(()),
        Some(45) => Err(Error::InvalidFilePath { field: "root" }),
        Some(46) => Err(Error::FileNotRegular),
        Some(47) => Err(Error::FileTooLarge {
            limit: FILE_TRANSFER_MAX_BYTES,
        }),
        Some(48) => Err(Error::InvalidFilePath { field: "path" }),
        _ => Err(Error::BackendUnavailable {
            backend_id: transport.backend_id.clone(),
        }),
    }
}

fn command(
    root: String,
    path: String,
    staging_path: String,
    temporary_name: String,
    expected_size: usize,
) -> ProcessCommand {
    ProcessCommand {
        command: "/usr/bin/python3".to_owned(),
        args: vec![
            "-c".to_owned(),
            WRITER.to_owned(),
            root,
            path,
            staging_path,
            temporary_name,
            expected_size.to_string(),
            FILE_TRANSFER_MAX_BYTES.to_string(),
        ],
        cwd: None,
        output_capture: ProcessOutputCapture::HardLimit { max_bytes: 4096 },
        timeout: WRITE_TIMEOUT,
        read_only: false,
    }
}

#[cfg(test)]
#[path = "_tests_/regular_file_write_tests.rs"]
mod regular_file_write_tests;
