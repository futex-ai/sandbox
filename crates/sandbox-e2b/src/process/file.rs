//! Durable provider-log reading through a bounded helper command.

use std::time::Duration;

use base64::{Engine, engine::general_purpose::STANDARD};
use sandbox_interface::{Error, ResourceKind, Result};

use super::{
    connect::ConnectProcessTransport,
    types::{
        ProcessCommand, ProcessConnection, ProcessFileChunk, ProcessFileValidation,
        ProcessOutputCapture,
    },
};

const VALIDATION_TIMEOUT: Duration = Duration::from_secs(10);

pub(super) async fn read(
    transport: &ConnectProcessTransport,
    connection: ProcessConnection,
    path: String,
    offset: u64,
    max_bytes: usize,
    timeout: Duration,
) -> Result<ProcessFileChunk> {
    let output = transport
        .run_helper(connection, read_command(path, offset, max_bytes), timeout)
        .await?;
    if output.exit_code != Some(0) {
        return Err(Error::NotFound {
            resource: ResourceKind::Terminal,
        });
    }
    let text = match std::str::from_utf8(&output.bytes) {
        Ok(text) => text,
        Err(source) => {
            return Err(Error::internal_with(
                source,
                "decode provider file response",
            ));
        }
    };
    let Some((size, encoded)) = text.split_once('\n') else {
        return Err(Error::internal_message(
            "provider file response omitted size marker",
        ));
    };
    let total_size = match size.trim().parse::<u64>() {
        Ok(size) => size,
        Err(source) => return Err(Error::internal_with(source, "parse provider file size")),
    };
    let bytes = match STANDARD.decode(encoded.trim()) {
        Ok(bytes) => bytes,
        Err(source) => return Err(Error::internal_with(source, "decode provider file bytes")),
    };
    Ok(ProcessFileChunk { bytes, total_size })
}

fn read_command(path: String, offset: u64, max_bytes: usize) -> ProcessCommand {
    let encoded_path = STANDARD.encode(path);
    let script = format!(
        "p=$(printf %s '{encoded_path}' | base64 -d); \
         test -f \"$p\" || exit 44; \
         b=$(dd if=\"$p\" bs=1 skip={offset} count={max_bytes} status=none 2>/dev/null | base64 -w0); \
         s=$(wc -c < \"$p\" 2>/dev/null || printf 0); \
         printf '%s\\n%s' \"$s\" \"$b\""
    );
    ProcessCommand {
        command: "/bin/sh".to_owned(),
        args: vec!["-c".to_owned(), script],
        cwd: None,
        output_capture: ProcessOutputCapture::HardLimit {
            max_bytes: max_bytes.saturating_mul(2).saturating_add(128),
        },
        timeout: Duration::from_secs(300),
        read_only: false,
    }
}

pub(super) async fn validate(
    transport: &ConnectProcessTransport,
    connection: ProcessConnection,
    root: String,
    path: String,
    allow_missing: bool,
) -> Result<ProcessFileValidation> {
    let output = transport
        .run_helper(
            connection,
            validation_command(root, path, allow_missing),
            VALIDATION_TIMEOUT,
        )
        .await?;
    if output.exit_code == Some(44) {
        return Err(Error::NotFound {
            resource: ResourceKind::File,
        });
    }
    if output.exit_code != Some(0) {
        return Err(Error::InvalidFilePath { field: "path" });
    }
    decode_validation(&output.bytes)
}

fn validation_command(root: String, path: String, allow_missing: bool) -> ProcessCommand {
    let encoded_root = STANDARD.encode(root);
    let encoded_path = STANDARD.encode(path);
    let missing = if allow_missing { "1" } else { "0" };
    let script = format!(
        "r=$(printf %s '{encoded_root}' | base64 -d); \
         p=$(printf %s '{encoded_path}' | base64 -d); \
         cr=$(realpath -e -- \"$r\") || exit 45; test -d \"$cr\" || exit 45; \
         set -f; oi=$IFS; IFS='/'; set -- $p; IFS=$oi; sl=0; c=\"$cr\"; \
         for s do c=\"$c/$s\"; test ! -L \"$c\" || sl=1; done; \
         j=\"$cr/$p\"; e=1; \
         if test -e \"$j\" || test -L \"$j\"; then \
         if test \"$sl\" = 1; then cp=$(realpath -m -- \"$j\" 2>/dev/null) || cp=\"$j\"; \
         else cp=$(realpath -e -- \"$j\") || exit 45; fi; \
         else e=0; test {missing} = 1 || exit 44; cp=$(realpath -m -- \"$j\") || exit 45; fi; \
         rg=0; sz=0; if test \"$e\" = 1 && test -f \"$cp\"; then rg=1; sz=$(wc -c < \"$cp\"); fi; \
         printf '%s\\n%s\\n%s\\n%s\\n%s\\n%s' \
         \"$(printf %s \"$cr\" | base64 -w0)\" \"$(printf %s \"$cp\" | base64 -w0)\" \
         \"$e\" \"$rg\" \"$sl\" \"$sz\""
    );
    ProcessCommand {
        command: "/bin/sh".to_owned(),
        args: vec!["-c".to_owned(), script],
        cwd: None,
        output_capture: ProcessOutputCapture::HardLimit {
            max_bytes: 2 * 4096 + 128,
        },
        timeout: VALIDATION_TIMEOUT,
        read_only: false,
    }
}

fn decode_validation(bytes: &[u8]) -> Result<ProcessFileValidation> {
    let text = match std::str::from_utf8(bytes) {
        Ok(text) => text,
        Err(source) => {
            return Err(Error::internal_with(
                source,
                "decode file validation response",
            ));
        }
    };
    let mut lines = text.lines();
    let canonical_root = decode_path(lines.next(), "root")?;
    let canonical_path = decode_path(lines.next(), "path")?;
    let exists = lines.next() == Some("1");
    let regular = lines.next() == Some("1");
    let symlink = lines.next() == Some("1");
    let size = match lines.next().unwrap_or_default().parse::<u64>() {
        Ok(size) => size,
        Err(source) => return Err(Error::internal_with(source, "parse validated file size")),
    };
    Ok(ProcessFileValidation {
        canonical_root,
        canonical_path,
        exists,
        regular,
        symlink,
        size,
    })
}

fn decode_path(value: Option<&str>, field: &'static str) -> Result<String> {
    let Some(value) = value else {
        return Err(Error::InvalidFilePath { field });
    };
    let bytes = match STANDARD.decode(value) {
        Ok(bytes) => bytes,
        Err(source) => return Err(Error::internal_with(source, "decode validated file path")),
    };
    match String::from_utf8(bytes) {
        Ok(path) => Ok(path),
        Err(source) => Err(Error::internal_with(
            source,
            "decode validated file path text",
        )),
    }
}

#[cfg(test)]
#[path = "_tests_/file_tests.rs"]
mod file_tests;
