//! Public file bounds, chunk integrity, and uncertain writer outcomes.

use sandbox_interface::{
    BackendReadFileRequest, BackendWriteFileRequest, Error, FILE_TRANSFER_MAX_BYTES, ProviderRef,
    SandboxFiles,
};

use super::support::fixture::Fixture;

fn write(bytes: Vec<u8>) -> BackendWriteFileRequest {
    BackendWriteFileRequest {
        sandbox_provider_ref: ProviderRef::new("bx_abcdefgh"),
        root: "/workspace".to_owned(),
        path: "file".to_owned(),
        bytes,
    }
}
fn read(maximum: usize, offset: u64) -> BackendReadFileRequest {
    BackendReadFileRequest {
        sandbox_provider_ref: ProviderRef::new("bx_abcdefgh"),
        root: "/workspace".to_owned(),
        path: "file".to_owned(),
        offset,
        max_bytes: maximum,
    }
}

#[tokio::test]
async fn native_chunks_preserve_large_files_offsets_full_size_and_normal_cleanup() {
    let fixture = Fixture::start().await;
    let bytes = (0..6 * 1024 * 1024 + 17)
        .map(|n| (n % 251) as u8)
        .collect::<Vec<_>>();
    fixture
        .backend
        .write_file(write(bytes.clone()))
        .await
        .unwrap();
    let output = fixture
        .backend
        .read_file(read(5 * 1024 * 1024 + 3, 7))
        .await
        .unwrap();
    assert_eq!(output.total_size, bytes.len() as u64);
    assert_eq!(output.bytes, bytes[7..5 * 1024 * 1024 + 10]);
    let requests = fixture.captured();
    assert!(
        requests
            .iter()
            .filter(|r| r.method == "GET" && r.path.contains("/files?"))
            .all(|r| r.path.contains(".sandbox-boat"))
    );
    assert_eq!(fixture.state.lock().unwrap().runtime.staging.len(), 0);
}

#[tokio::test]
async fn path_and_transfer_bounds_fail_before_provider_access() {
    let fixture = Fixture::start().await;
    let mut invalid = read(1, 0);
    invalid.root = "/workspace/../etc".to_owned();
    assert!(matches!(
        fixture.backend.read_file(invalid).await,
        Err(Error::InvalidFilePath { field: "root" })
    ));
    assert!(matches!(
        fixture
            .backend
            .read_file(read(FILE_TRANSFER_MAX_BYTES + 1, 0))
            .await,
        Err(Error::FileTooLarge { .. })
    ));
    let mut invalid = write(Vec::new());
    invalid.path = "../file".to_owned();
    assert!(matches!(
        fixture.backend.write_file(invalid).await,
        Err(Error::InvalidFilePath { field: "path" })
    ));
    assert_eq!(fixture.captured().len(), 0);
}

#[tokio::test]
async fn altered_chunks_fail_integrity_and_unconfirmed_writer_retains_uncertainty() {
    let fixture = Fixture::start().await;
    fixture.state.lock().unwrap().runtime.upload_tamper = true;
    assert!(matches!(
        fixture.backend.write_file(write(b"payload".to_vec())).await,
        Err(Error::BackendUnavailable { .. })
    ));
    assert_eq!(fixture.state.lock().unwrap().runtime.files.len(), 0);
    {
        let mut state = fixture.state.lock().unwrap();
        state.runtime.upload_tamper = false;
        state.runtime.write_failure = Some(49);
        state.runtime.cleanup_failure = true;
    }
    assert!(matches!(
        fixture.backend.write_file(write(b"payload".to_vec())).await,
        Err(Error::FileWriteUnconfirmed)
    ));
    assert!(
        fixture
            .state
            .lock()
            .unwrap()
            .runtime
            .actions
            .contains(&"reconcile".to_owned())
    );
    {
        let mut state = fixture.state.lock().unwrap();
        state.runtime.write_failure = Some(46);
    }
    assert!(matches!(
        fixture.backend.write_file(write(b"payload".to_vec())).await,
        Err(Error::FileNotRegular)
    ));
}

#[tokio::test]
async fn same_size_download_tamper_cannot_pass_the_export_digest() {
    let fixture = Fixture::start().await;
    {
        let mut state = fixture.state.lock().unwrap();
        state
            .runtime
            .files
            .insert("/workspace/file".to_owned(), b"payload".to_vec());
        state.runtime.read_tamper = true;
    }
    assert!(matches!(
        fixture.backend.read_file(read(7, 0)).await,
        Err(Error::BackendUnavailable { .. })
    ));
    assert_eq!(fixture.state.lock().unwrap().runtime.staging.len(), 0);
}
