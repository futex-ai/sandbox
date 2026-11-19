//! File acknowledgments preserve identity across Boat's home-relative paths.

use std::{sync::Arc, time::Duration};

use serde_json::{Value, json};
use unimock::{MockFn, Unimock, matching};

use crate::{
    error::Error,
    runtime::{api::BoatRuntimeApi, client::HttpsBoatRuntimeApi},
    transport::runtime::BoatRuntimeTransportMock,
};

const RELATIVE: &str = ".sandbox-boat/0123456789abcdef0123456789abcdef/bundle.json";
const TIMEOUT: Duration = Duration::from_secs(1);

fn requested_path() -> String {
    format!("/home/user/{RELATIVE}")
}

fn reply(kind: &str, path: &str) -> Value {
    json!({
        "ok": true,
        "success": true,
        "type": kind,
        "path": path,
        "encoding": "base64",
        "size": 7,
        "content": "cGF5bG9hZA==",
    })
}

fn api(reply: Value) -> HttpsBoatRuntimeApi {
    let transport = Unimock::new(
        BoatRuntimeTransportMock::request
            .some_call(matching!(_))
            .returns(Ok(serde_json::to_vec(&reply).unwrap())),
    );
    HttpsBoatRuntimeApi::new(Arc::new(transport))
}

async fn reject_path(path: &str) {
    let uploaded = api(reply("file.written", path))
        .upload(
            "bx_abcdefgh",
            requested_path(),
            b"payload".to_vec(),
            TIMEOUT,
        )
        .await;
    assert!(
        matches!(uploaded, Err(Error::InvalidResponse)),
        "upload acknowledgment {path:?}: {uploaded:?}"
    );
    let downloaded = api(reply("file.read", path))
        .download("bx_abcdefgh", requested_path(), TIMEOUT)
        .await;
    assert!(
        matches!(downloaded, Err(Error::InvalidResponse)),
        "download acknowledgment {path:?}: {downloaded:?}"
    );
}

#[tokio::test]
async fn upload_and_download_accept_home_relative_and_normalized_acknowledgments() {
    for path in [
        requested_path(),
        RELATIVE.to_owned(),
        format!("./{RELATIVE}"),
        format!("/home//user/./{RELATIVE}"),
        RELATIVE.replace("/bundle.json", "//./bundle.json"),
    ] {
        let uploaded = api(reply("file.written", &path))
            .upload(
                "bx_abcdefgh",
                requested_path(),
                b"payload".to_vec(),
                TIMEOUT,
            )
            .await;
        assert!(matches!(uploaded, Ok(())), "{path:?}: {uploaded:?}");
        let downloaded = api(reply("file.read", &path))
            .download("bx_abcdefgh", requested_path(), TIMEOUT)
            .await
            .unwrap();
        assert_eq!(downloaded, b"payload", "{path:?}");
    }
}

#[tokio::test]
async fn upload_and_download_reject_acknowledgments_for_other_files() {
    for path in [
        "bundle.json".to_owned(),
        RELATIVE.replace("bundle.json", "other.json"),
        requested_path().replace("bundle.json", "other.json"),
        RELATIVE.replace(
            "0123456789abcdef0123456789abcdef",
            "fedcba9876543210fedcba9876543210",
        ),
    ] {
        reject_path(&path).await;
    }
}

#[tokio::test]
async fn upload_and_download_reject_acknowledgments_outside_home() {
    for path in [
        format!("/tmp/{RELATIVE}"),
        format!("/home/user-other/{RELATIVE}"),
        format!("/{RELATIVE}"),
    ] {
        reject_path(&path).await;
    }
}

#[tokio::test]
async fn upload_and_download_reject_parent_parts_even_when_the_file_would_match() {
    for path in [
        format!("../user/{RELATIVE}"),
        format!("/home/user/../user/{RELATIVE}"),
        RELATIVE.replace("/bundle.json", "/nested/../bundle.json"),
        requested_path().replace("/bundle.json", "/nested/../bundle.json"),
    ] {
        reject_path(&path).await;
    }
}

#[tokio::test]
async fn home_relative_acknowledgments_keep_success_encoding_and_size_checks() {
    for (field, value) in [
        ("ok", json!(false)),
        ("success", json!(false)),
        ("type", json!("file.deleted")),
        ("encoding", json!("utf8")),
        ("size", json!(8)),
    ] {
        let mut upload = reply("file.written", RELATIVE);
        upload[field] = value.clone();
        assert!(
            matches!(
                api(upload)
                    .upload(
                        "bx_abcdefgh",
                        requested_path(),
                        b"payload".to_vec(),
                        TIMEOUT
                    )
                    .await,
                Err(Error::InvalidResponse)
            ),
            "upload field {field}"
        );
        let mut download = reply("file.read", RELATIVE);
        download[field] = value;
        assert!(
            matches!(
                api(download)
                    .download("bx_abcdefgh", requested_path(), TIMEOUT)
                    .await,
                Err(Error::InvalidResponse)
            ),
            "download field {field}"
        );
    }
}
