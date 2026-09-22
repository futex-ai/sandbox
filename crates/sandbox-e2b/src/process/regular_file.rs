//! Atomic descriptor-relative regular-file reads inside an E2B sandbox.

use std::{array::TryFromSliceError, mem::size_of};

use sandbox_interface::{Error, FILE_TRANSFER_MAX_BYTES, ResourceKind, Result};

use crate::trusted_python;

use super::{
    connect::ConnectProcessTransport,
    types::{
        ProcessCommand, ProcessConnection, ProcessFileChunk, ProcessOutputCapture,
        ProcessRegularFileRequest,
    },
};

const SIZE_HEADER_BYTES: usize = size_of::<u64>();
const READER: &str = r#"import errno
import os
import stat
import struct
import sys

NOT_FOUND = 44
INVALID_ROOT = 45
NOT_REGULAR = 46
TOO_LARGE = 47
INVALID_PATH = 48
FAILED = 49

def stop(code):
    os._exit(code)

def open_below(directory, name, flags, missing):
    try:
        return os.open(name, flags, dir_fd=directory)
    except FileNotFoundError:
        stop(missing)
    except OSError as error:
        if error.errno in (errno.ELOOP, errno.ENOTDIR):
            stop(NOT_REGULAR)
        stop(FAILED)

def write_all(value):
    remaining = memoryview(value)
    while remaining:
        written = os.write(1, remaining)
        remaining = remaining[written:]

try:
    root = sys.argv[1]
    path = sys.argv[2]
    offset = int(sys.argv[3])
    maximum = int(sys.argv[4])
    transfer_limit = int(sys.argv[5])
    root_parts = [part for part in root.split('/') if part]
    path_parts = path.split('/')
    if root != '/' + '/'.join(root_parts):
        stop(INVALID_ROOT)
    if not path_parts or any(part in ('', '.', '..') for part in path_parts):
        stop(INVALID_PATH)
    if offset < 0 or maximum < 0 or transfer_limit < 0:
        stop(INVALID_PATH)
    directory_flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
    directory = os.open('/', directory_flags)
    try:
        for part in root_parts:
            child = open_below(directory, part, directory_flags, INVALID_ROOT)
            os.close(directory)
            directory = child
        for part in path_parts[:-1]:
            child = open_below(directory, part, directory_flags, NOT_FOUND)
            os.close(directory)
            directory = child
        file_flags = os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC
        opened = open_below(directory, path_parts[-1], file_flags, NOT_FOUND)
    finally:
        os.close(directory)
    try:
        metadata = os.fstat(opened)
        if not stat.S_ISREG(metadata.st_mode):
            stop(NOT_REGULAR)
        if metadata.st_size > transfer_limit:
            stop(TOO_LARGE)
        write_all(struct.pack('>Q', metadata.st_size))
        position = offset
        remaining = maximum
        while remaining and position < metadata.st_size:
            chunk = os.pread(opened, min(remaining, 65536), position)
            if not chunk:
                break
            write_all(chunk)
            position += len(chunk)
            remaining -= len(chunk)
    finally:
        os.close(opened)
except (IndexError, ValueError):
    stop(INVALID_PATH)
except OSError:
    stop(FAILED)
"#;

pub(super) async fn read(
    transport: &ConnectProcessTransport,
    connection: ProcessConnection,
    request: ProcessRegularFileRequest,
) -> Result<ProcessFileChunk> {
    let max_bytes = request.max_bytes;
    let timeout = request.timeout;
    let completion_deadline = request.completion_deadline;
    let command = command(request);
    let output = match completion_deadline {
        Some(deadline) => {
            transport
                .run_helper_before(connection, command, deadline)
                .await?
        }
        None => transport.run_helper(connection, command, timeout).await?,
    };
    match output.exit_code {
        Some(0) => decode(&output.bytes, max_bytes),
        Some(44) => Err(Error::NotFound {
            resource: ResourceKind::File,
        }),
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

fn command(request: ProcessRegularFileRequest) -> ProcessCommand {
    ProcessCommand {
        command: trusted_python::EXECUTABLE.to_owned(),
        args: trusted_python::command_args(
            READER,
            [
                request.root,
                request.path,
                request.offset.to_string(),
                request.max_bytes.to_string(),
                FILE_TRANSFER_MAX_BYTES.to_string(),
            ],
        ),
        cwd: None,
        output_capture: ProcessOutputCapture::HardLimit {
            max_bytes: request.max_bytes.saturating_add(SIZE_HEADER_BYTES),
        },
        timeout: request.timeout,
        read_only: true,
    }
}

fn decode(bytes: &[u8], max_bytes: usize) -> Result<ProcessFileChunk> {
    if bytes.len() < SIZE_HEADER_BYTES {
        return Err(Error::internal_message(
            "atomic regular-file response omitted size header",
        ));
    }
    let header: [u8; SIZE_HEADER_BYTES] = bytes[..SIZE_HEADER_BYTES]
        .try_into()
        .map_err(map_size_header_error)?;
    let content = bytes[SIZE_HEADER_BYTES..].to_vec();
    if content.len() > max_bytes {
        return Err(Error::ReadOnlyOutputTooLarge);
    }
    Ok(ProcessFileChunk {
        bytes: content,
        total_size: u64::from_be_bytes(header),
    })
}

fn map_size_header_error(source: TryFromSliceError) -> Error {
    Error::internal_with(source, "decode atomic regular-file size header")
}

#[cfg(test)]
#[path = "_tests_/regular_file_tests.rs"]
mod regular_file_tests;
