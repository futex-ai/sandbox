//! The complete public file ceiling uses protected native chunks.

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use sandbox_interface::{BackendWriteFileRequest, Error, FILE_TRANSFER_MAX_BYTES, ProviderRef};
use unimock::{MockFn, Unimock, matching};

use crate::{
    config::{BoatAdapterConfig, BoatProfile},
    runtime::{
        api::{BoatRuntimeApiMock, HelperOutput},
        configured::{BoatRuntime, BoatTrustedRuntime},
    },
};

#[tokio::test]
async fn a_full_size_transfer_uses_sixty_four_chunks_and_rejects_only_the_next_byte() {
    let chunks = Arc::new(Mutex::new(Vec::new()));
    let uploaded = chunks.clone();
    let api = Unimock::new((
        BoatRuntimeApiMock::helper
            .each_call(matching!(_, _, _))
            .answers(&|_, _, _, _| {
                Ok(HelperOutput {
                    exit_code: Some(0),
                    stdout: String::new(),
                    normal: true,
                })
            }),
        BoatRuntimeApiMock::upload
            .each_call(matching!(_, _, _, _))
            .answers_arc(Arc::new(move |_, _, path, bytes, _| {
                if !path.ends_with("bundle.json") {
                    uploaded.lock().unwrap().push(bytes.len());
                }
                Ok(())
            })),
    ));
    let config = BoatAdapterConfig::new(
        "boat",
        "https://boat.dev",
        "key",
        BTreeMap::from([(
            "general".to_owned(),
            BoatProfile {
                provider_size: "default".to_owned(),
                allow_public_egress: true,
                denied_destinations: Vec::new(),
            },
        )]),
    )
    .unwrap();
    let runtime = BoatTrustedRuntime::new(config, Arc::new(api));
    let request = |bytes| BackendWriteFileRequest {
        sandbox_provider_ref: ProviderRef::new("bx_abcdefgh"),
        root: "/workspace".to_owned(),
        path: "file".to_owned(),
        bytes,
    };
    assert!(matches!(
        runtime
            .write_file(request(vec![0; FILE_TRANSFER_MAX_BYTES]))
            .await,
        Ok(())
    ));
    assert_eq!(*chunks.lock().unwrap(), vec![4 * 1024 * 1024; 64]);
    assert!(matches!(
        runtime
            .write_file(request(vec![0; FILE_TRANSFER_MAX_BYTES + 1]))
            .await,
        Err(Error::FileTooLarge {
            limit: FILE_TRANSFER_MAX_BYTES
        })
    ));
    assert_eq!(chunks.lock().unwrap().len(), 64);
}
