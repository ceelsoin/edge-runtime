use std::io;
use std::sync::Arc;

use anyhow::Error;
use rustls::ServerConfig;
use tokio_rustls::TlsAcceptor;

use crate::TlsConfig;

/// Build a TLS acceptor from certificate and key files.
pub fn build_tls_acceptor(config: &TlsConfig) -> Result<TlsAcceptor, Error> {
    let cert_pem = std::fs::read(&config.cert_path)
        .map_err(|e| anyhow::anyhow!("failed to read TLS cert '{}': {}", config.cert_path, e))?;
    let key_pem = std::fs::read(&config.key_path)
        .map_err(|e| anyhow::anyhow!("failed to read TLS key '{}': {}", config.key_path, e))?;

    let certs = rustls_pemfile::certs(&mut io::BufReader::new(cert_pem.as_slice()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| anyhow::anyhow!("failed to parse TLS certificates: {}", e))?;

    let key = rustls_pemfile::private_key(&mut io::BufReader::new(key_pem.as_slice()))
        .map_err(|e| anyhow::anyhow!("failed to parse TLS private key: {}", e))?
        .ok_or_else(|| anyhow::anyhow!("no private key found in TLS key file"))?;

    let mut tls_config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| anyhow::anyhow!("invalid TLS config: {}", e))?;

    tls_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    Ok(TlsAcceptor::from(Arc::new(tls_config)))
}
