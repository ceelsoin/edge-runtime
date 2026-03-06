pub mod admin_router;
pub mod body_limits;
pub mod graceful;
pub mod ingress_router;
pub mod router;
pub mod service;
pub mod tls;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Error;
use hyper_util::rt::TokioIo;
use tokio::net::{TcpListener, UnixListener};
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use functions::registry::FunctionRegistry;

use crate::admin_router::AdminRouter;
use crate::ingress_router::IngressRouter;
use crate::service::EdgeService;

// Re-export for convenience
pub use crate::body_limits::BodyLimitsConfig;

// ─────────────────────────────────────────────────────────────────────────────
// Configuration Types
// ─────────────────────────────────────────────────────────────────────────────

/// Dual-listener server configuration.
#[derive(Debug, Clone)]
pub struct DualServerConfig {
    pub admin: AdminListenerConfig,
    pub ingress: IngressListenerConfig,
    pub graceful_exit_deadline_secs: u64,
    /// Maximum concurrent connections across all listeners.
    pub max_connections: usize,
}

/// Admin listener configuration (TCP only).
#[derive(Debug, Clone)]
pub struct AdminListenerConfig {
    /// Address to bind (default: 0.0.0.0:9000)
    pub addr: SocketAddr,
    /// API key for authentication. None = no auth (dev mode).
    pub api_key: Option<String>,
    /// Optional TLS configuration.
    pub tls: Option<TlsConfig>,
    /// Body size limits.
    pub body_limits: BodyLimitsConfig,
}

/// Ingress listener configuration (TCP or Unix socket).
#[derive(Debug, Clone)]
pub struct IngressListenerConfig {
    /// Listener type: TCP or Unix socket.
    pub listener_type: IngressListenerType,
    /// Optional TLS (only for TCP).
    pub tls: Option<TlsConfig>,
    /// Rate limit in requests per second.
    pub rate_limit_rps: Option<u64>,
    /// Body size limits.
    pub body_limits: BodyLimitsConfig,
}

/// Ingress listener type.
#[derive(Debug, Clone)]
pub enum IngressListenerType {
    /// TCP socket with address.
    Tcp(SocketAddr),
    /// Unix domain socket with path.
    Unix(PathBuf),
}

/// Legacy server configuration (single listener).
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub addr: SocketAddr,
    pub tls: Option<TlsConfig>,
    pub rate_limit_rps: Option<u64>,
    pub graceful_exit_deadline_secs: u64,
    /// Body size limits.
    pub body_limits: BodyLimitsConfig,
    /// Maximum concurrent connections.
    pub max_connections: usize,
}

/// TLS configuration.
#[derive(Debug, Clone)]
pub struct TlsConfig {
    pub cert_path: String,
    pub key_path: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// Dual Server (New Architecture)
// ─────────────────────────────────────────────────────────────────────────────

/// Start the dual-listener HTTP server.
///
/// - Admin listener on `config.admin.addr` (default port 9000) with API key auth
/// - Ingress listener on TCP port or Unix socket for function requests
pub async fn run_dual_server(
    config: DualServerConfig,
    registry: Arc<FunctionRegistry>,
    shutdown: CancellationToken,
) -> Result<(), Error> {
    // Warn if no API key configured
    if config.admin.api_key.is_none() {
        warn!(
            "admin API running without authentication (no --api-key set). \
             This is insecure for production use."
        );
    }

    // Create connection semaphore shared across all listeners
    let connection_semaphore = Arc::new(Semaphore::new(config.max_connections));
    info!(
        "connection limit set to {} concurrent connections",
        config.max_connections
    );

    // Create routers with shared registry and body limits
    let admin_router = AdminRouter::new(
        registry.clone(),
        config.admin.api_key.clone(),
        config.admin.body_limits,
    );
    let ingress_router = IngressRouter::new(registry.clone(), config.ingress.body_limits);

    // Spawn admin listener
    let admin_shutdown = shutdown.clone();
    let admin_config = config.admin.clone();
    let admin_semaphore = connection_semaphore.clone();
    let admin_handle = tokio::spawn(async move {
        if let Err(e) =
            run_admin_listener(admin_config, admin_router, admin_shutdown, admin_semaphore).await
        {
            error!("admin listener error: {}", e);
        }
    });

    // Spawn ingress listener
    let ingress_shutdown = shutdown.clone();
    let ingress_config = config.ingress.clone();
    let ingress_semaphore = connection_semaphore.clone();
    let ingress_handle = tokio::spawn(async move {
        if let Err(e) =
            run_ingress_listener(ingress_config, ingress_router, ingress_shutdown, ingress_semaphore)
                .await
        {
            error!("ingress listener error: {}", e);
        }
    });

    // Wait for shutdown signal
    shutdown.cancelled().await;
    info!("shutdown signal received, stopping listeners...");

    // Wait for listeners to finish with deadline
    let deadline = Duration::from_secs(config.graceful_exit_deadline_secs);
    let _ = tokio::time::timeout(deadline, async {
        let _ = admin_handle.await;
        let _ = ingress_handle.await;
    })
    .await;

    info!(
        "waited up to {}s for connections to drain",
        config.graceful_exit_deadline_secs
    );

    Ok(())
}

/// Run the admin listener (TCP only, with optional TLS).
async fn run_admin_listener(
    config: AdminListenerConfig,
    router: AdminRouter,
    shutdown: CancellationToken,
    connection_semaphore: Arc<Semaphore>,
) -> Result<(), Error> {
    let listener = TcpListener::bind(config.addr).await?;

    let tls_acceptor = if let Some(ref tls_config) = config.tls {
        Some(tls::build_tls_acceptor(tls_config)?)
    } else {
        None
    };

    let scheme = if tls_acceptor.is_some() {
        "https"
    } else {
        "http"
    };
    info!("admin API listening on {}://{}", scheme, config.addr);

    let svc = EdgeService::new(router);

    loop {
        tokio::select! {
            accepted = listener.accept() => {
                match accepted {
                    Ok((stream, peer_addr)) => {
                        // Try to acquire connection permit
                        let permit = match connection_semaphore.clone().try_acquire_owned() {
                            Ok(permit) => permit,
                            Err(_) => {
                                warn!("admin: connection limit reached, rejecting {}", peer_addr);
                                drop(stream);
                                continue;
                            }
                        };

                        let svc = svc.clone();
                        let tls_acceptor = tls_acceptor.clone();
                        tokio::spawn(async move {
                            // Permit is held for the duration of this task
                            let _permit = permit;

                            let maybe_stream = if let Some(acceptor) = tls_acceptor {
                                match acceptor.accept(stream).await {
                                    Ok(tls_stream) => tls::MaybeHttpsStream::TcpTls(tls_stream),
                                    Err(e) => {
                                        tracing::warn!("admin TLS handshake failed from {}: {}", peer_addr, e);
                                        return;
                                    }
                                }
                            } else {
                                tls::MaybeHttpsStream::TcpPlain(stream)
                            };

                            let io = TokioIo::new(maybe_stream);
                            let conn = hyper_util::server::conn::auto::Builder::new(
                                hyper_util::rt::TokioExecutor::new(),
                            );
                            if let Err(e) = conn.serve_connection(io, svc).await {
                                tracing::debug!("admin connection error from {}: {}", peer_addr, e);
                            }
                        });
                    }
                    Err(e) => {
                        error!("admin accept error: {}", e);
                    }
                }
            }
            _ = shutdown.cancelled() => {
                info!("admin listener stopping");
                break;
            }
        }
    }

    Ok(())
}

/// Run the ingress listener (TCP or Unix socket).
async fn run_ingress_listener(
    config: IngressListenerConfig,
    router: IngressRouter,
    shutdown: CancellationToken,
    connection_semaphore: Arc<Semaphore>,
) -> Result<(), Error> {
    match config.listener_type {
        IngressListenerType::Tcp(addr) => {
            run_tcp_ingress(addr, config.tls, router, shutdown, connection_semaphore).await
        }
        IngressListenerType::Unix(path) => {
            run_unix_ingress(path, router, shutdown, connection_semaphore).await
        }
    }
}

/// Run ingress on TCP socket.
async fn run_tcp_ingress(
    addr: SocketAddr,
    tls_config: Option<TlsConfig>,
    router: IngressRouter,
    shutdown: CancellationToken,
    connection_semaphore: Arc<Semaphore>,
) -> Result<(), Error> {
    let listener = TcpListener::bind(addr).await?;

    let tls_acceptor = if let Some(ref tls) = tls_config {
        Some(tls::build_tls_acceptor(tls)?)
    } else {
        None
    };

    let scheme = if tls_acceptor.is_some() {
        "https"
    } else {
        "http"
    };
    info!("ingress listening on {}://{}", scheme, addr);

    let svc = EdgeService::new(router);

    loop {
        tokio::select! {
            accepted = listener.accept() => {
                match accepted {
                    Ok((stream, peer_addr)) => {
                        // Try to acquire connection permit
                        let permit = match connection_semaphore.clone().try_acquire_owned() {
                            Ok(permit) => permit,
                            Err(_) => {
                                warn!("ingress: connection limit reached, rejecting {}", peer_addr);
                                drop(stream);
                                continue;
                            }
                        };

                        let svc = svc.clone();
                        let tls_acceptor = tls_acceptor.clone();
                        tokio::spawn(async move {
                            // Permit is held for the duration of this task
                            let _permit = permit;

                            let maybe_stream = if let Some(acceptor) = tls_acceptor {
                                match acceptor.accept(stream).await {
                                    Ok(tls_stream) => tls::MaybeHttpsStream::TcpTls(tls_stream),
                                    Err(e) => {
                                        tracing::warn!("ingress TLS handshake failed from {}: {}", peer_addr, e);
                                        return;
                                    }
                                }
                            } else {
                                tls::MaybeHttpsStream::TcpPlain(stream)
                            };

                            let io = TokioIo::new(maybe_stream);
                            let conn = hyper_util::server::conn::auto::Builder::new(
                                hyper_util::rt::TokioExecutor::new(),
                            );
                            if let Err(e) = conn.serve_connection(io, svc).await {
                                tracing::debug!("ingress connection error from {}: {}", peer_addr, e);
                            }
                        });
                    }
                    Err(e) => {
                        error!("ingress accept error: {}", e);
                    }
                }
            }
            _ = shutdown.cancelled() => {
                info!("ingress TCP listener stopping");
                break;
            }
        }
    }

    Ok(())
}

/// Run ingress on Unix socket.
async fn run_unix_ingress(
    path: PathBuf,
    router: IngressRouter,
    shutdown: CancellationToken,
    connection_semaphore: Arc<Semaphore>,
) -> Result<(), Error> {
    // Clean up stale socket file if exists
    if path.exists() {
        std::fs::remove_file(&path)?;
    }

    let listener = UnixListener::bind(&path)?;
    info!("ingress listening on unix:{}", path.display());

    let svc = EdgeService::new(router);
    let cleanup_path = path.clone();

    loop {
        tokio::select! {
            accepted = listener.accept() => {
                match accepted {
                    Ok((stream, _addr)) => {
                        // Try to acquire connection permit
                        let permit = match connection_semaphore.clone().try_acquire_owned() {
                            Ok(permit) => permit,
                            Err(_) => {
                                warn!("unix ingress: connection limit reached, rejecting connection");
                                drop(stream);
                                continue;
                            }
                        };

                        let svc = svc.clone();
                        tokio::spawn(async move {
                            // Permit is held for the duration of this task
                            let _permit = permit;

                            let maybe_stream = tls::MaybeHttpsStream::Unix(stream);
                            let io = TokioIo::new(maybe_stream);
                            let conn = hyper_util::server::conn::auto::Builder::new(
                                hyper_util::rt::TokioExecutor::new(),
                            );
                            if let Err(e) = conn.serve_connection(io, svc).await {
                                tracing::debug!("unix connection error: {}", e);
                            }
                        });
                    }
                    Err(e) => {
                        error!("unix accept error: {}", e);
                    }
                }
            }
            _ = shutdown.cancelled() => {
                info!("ingress Unix listener stopping");
                break;
            }
        }
    }

    // Cleanup socket file
    if let Err(e) = std::fs::remove_file(&cleanup_path) {
        warn!("failed to remove Unix socket {}: {}", cleanup_path.display(), e);
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Legacy Single Server (Backward Compatibility)
// ─────────────────────────────────────────────────────────────────────────────

/// Start the HTTP server and block until shutdown.
///
/// This is the legacy single-listener interface. For new deployments,
/// use `run_dual_server` instead.
pub async fn run_server(
    config: ServerConfig,
    registry: Arc<FunctionRegistry>,
    shutdown: CancellationToken,
) -> Result<(), Error> {
    let router = router::Router::new(registry, config.body_limits);
    let svc = service::EdgeService::new(router);

    let listener = TcpListener::bind(config.addr).await?;

    // Create connection semaphore
    let connection_semaphore = Arc::new(Semaphore::new(config.max_connections));
    info!(
        "connection limit set to {} concurrent connections",
        config.max_connections
    );

    // Optional TLS acceptor
    let tls_acceptor = if let Some(ref tls_config) = config.tls {
        Some(tls::build_tls_acceptor(tls_config)?)
    } else {
        None
    };

    let scheme = if tls_acceptor.is_some() { "https" } else { "http" };
    info!("edge-runtime listening on {}://{}", scheme, config.addr);

    loop {
        tokio::select! {
            accepted = listener.accept() => {
                match accepted {
                    Ok((stream, peer_addr)) => {
                        // Try to acquire connection permit
                        let permit = match connection_semaphore.clone().try_acquire_owned() {
                            Ok(permit) => permit,
                            Err(_) => {
                                warn!("connection limit reached, rejecting {}", peer_addr);
                                drop(stream);
                                continue;
                            }
                        };

                        let svc = svc.clone();
                        let tls_acceptor = tls_acceptor.clone();
                        tokio::spawn(async move {
                            // Permit is held for the duration of this task
                            let _permit = permit;

                            let maybe_stream = if let Some(acceptor) = tls_acceptor {
                                match acceptor.accept(stream).await {
                                    Ok(tls_stream) => tls::MaybeHttpsStream::TcpTls(tls_stream),
                                    Err(e) => {
                                        tracing::warn!("TLS handshake failed from {}: {}", peer_addr, e);
                                        return;
                                    }
                                }
                            } else {
                                tls::MaybeHttpsStream::TcpPlain(stream)
                            };

                            let io = TokioIo::new(maybe_stream);
                            let conn = hyper_util::server::conn::auto::Builder::new(
                                hyper_util::rt::TokioExecutor::new(),
                            );
                            if let Err(e) = conn.serve_connection(io, svc).await {
                                // Connection errors are normal (client disconnects, etc.)
                                tracing::debug!("connection error from {}: {}", peer_addr, e);
                            }
                        });
                    }
                    Err(e) => {
                        error!("failed to accept connection: {}", e);
                    }
                }
            }
            _ = shutdown.cancelled() => {
                info!("shutdown signal received, stopping server...");
                break;
            }
        }
    }

    // Graceful shutdown: wait for in-flight connections
    info!(
        "waiting up to {}s for connections to drain",
        config.graceful_exit_deadline_secs
    );
    tokio::time::sleep(std::time::Duration::from_secs(
        config.graceful_exit_deadline_secs,
    ))
    .await;

    Ok(())
}
