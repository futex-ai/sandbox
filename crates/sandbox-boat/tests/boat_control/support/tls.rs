//! Local verified TLS listener using an additional test trust root.

use std::sync::Arc;

use tokio::net::TcpListener;
use tokio_rustls::{
    TlsAcceptor,
    rustls::{ServerConfig, crypto::aws_lc_rs, pki_types::PrivatePkcs8KeyDer},
};

pub(super) async fn listener() -> (TcpListener, TlsAcceptor, Vec<u8>) {
    let identity =
        rcgen::generate_simple_self_signed(vec!["localhost".to_owned(), "127.0.0.1".to_owned()])
            .unwrap();
    let server = ServerConfig::builder_with_provider(Arc::new(aws_lc_rs::default_provider()))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(
            vec![identity.cert.der().clone()],
            PrivatePkcs8KeyDer::from(identity.signing_key.serialize_der()).into(),
        )
        .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    (
        listener,
        TlsAcceptor::from(Arc::new(server)),
        identity.cert.pem().into_bytes(),
    )
}
