use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use clap::{Args, ValueEnum};
use tokio_util::sync::CancellationToken;
use tracing::info;

use runtime_core::isolate::IsolateConfig;

#[derive(Debug, Clone, Copy, ValueEnum)]
enum SourceMapMode {
    None,
    Inline,
}

#[derive(Args)]
pub struct StartArgs {
    // ─────────────────────────────────────────────────────────────────────────
    // Admin Listener Configuration (port 9000 by default)
    // ─────────────────────────────────────────────────────────────────────────
    /// Admin API host
    #[arg(long, default_value = "0.0.0.0", env = "EDGE_RUNTIME_ADMIN_HOST")]
    admin_host: String,

    /// Admin API port
    #[arg(long, default_value_t = 9000, env = "EDGE_RUNTIME_ADMIN_PORT")]
    admin_port: u16,

    /// API key for admin endpoint authentication (required in production)
    #[arg(long, env = "EDGE_RUNTIME_API_KEY")]
    api_key: Option<String>,

    /// TLS certificate file path for admin API
    #[arg(long, env = "EDGE_RUNTIME_ADMIN_TLS_CERT")]
    admin_tls_cert: Option<String>,

    /// TLS private key file path for admin API
    #[arg(long, env = "EDGE_RUNTIME_ADMIN_TLS_KEY")]
    admin_tls_key: Option<String>,

    // ─────────────────────────────────────────────────────────────────────────
    // Ingress Listener Configuration (TCP port or Unix socket)
    // ─────────────────────────────────────────────────────────────────────────
    /// Ingress IP address to bind (for TCP mode)
    #[arg(long, default_value = "0.0.0.0", env = "EDGE_RUNTIME_HOST")]
    host: String,

    /// Ingress port to listen on (mutually exclusive with --unix-socket)
    #[arg(short, long, env = "EDGE_RUNTIME_PORT")]
    port: Option<u16>,

    /// Unix socket path for ingress (mutually exclusive with --port)
    #[arg(long, env = "EDGE_RUNTIME_UNIX_SOCKET")]
    unix_socket: Option<PathBuf>,

    /// TLS certificate file path for ingress (TCP only)
    #[arg(long, env = "EDGE_RUNTIME_TLS_CERT")]
    tls_cert: Option<String>,

    /// TLS private key file path for ingress (TCP only)
    #[arg(long, env = "EDGE_RUNTIME_TLS_KEY")]
    tls_key: Option<String>,

    // ─────────────────────────────────────────────────────────────────────────
    // Common Options
    // ─────────────────────────────────────────────────────────────────────────
    /// Rate limit (requests per second, 0 = unlimited)
    #[arg(long, default_value_t = 0, env = "EDGE_RUNTIME_RATE_LIMIT")]
    rate_limit: u64,

    /// Graceful shutdown deadline in seconds
    #[arg(long, default_value_t = 30)]
    graceful_exit_timeout: u64,

    /// Default max heap size per isolate in MiB (0 = unlimited)
    #[arg(long, default_value_t = 128, env = "EDGE_RUNTIME_MAX_HEAP_MIB")]
    max_heap_mib: u64,

    /// Default CPU time limit per request in ms (0 = unlimited)
    #[arg(long, default_value_t = 50000, env = "EDGE_RUNTIME_CPU_TIME_LIMIT_MS")]
    cpu_time_limit_ms: u64,

    /// Default wall clock timeout per request in ms (0 = unlimited)
    #[arg(long, default_value_t = 60000, env = "EDGE_RUNTIME_WALL_CLOCK_TIMEOUT_MS")]
    wall_clock_timeout_ms: u64,

    /// Source map handling for modules loaded from eszip
    #[arg(long, value_enum, default_value = "none", env = "EDGE_RUNTIME_SOURCE_MAP")]
    sourcemap: SourceMapMode,
}

pub fn run(args: StartArgs) -> Result<(), anyhow::Error> {
    // Validate mutually exclusive options
    if args.port.is_some() && args.unix_socket.is_some() {
        return Err(anyhow::anyhow!(
            "--port and --unix-socket are mutually exclusive"
        ));
    }

    // Warn if TLS specified with Unix socket
    if args.unix_socket.is_some() && (args.tls_cert.is_some() || args.tls_key.is_some()) {
        tracing::warn!("TLS options (--tls-cert, --tls-key) ignored for Unix socket ingress");
    }

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("edge-rt")
        .build()?;

    runtime.block_on(async {
        let shutdown = CancellationToken::new();

        let default_config = IsolateConfig {
            max_heap_size_bytes: (args.max_heap_mib as usize) * 1024 * 1024,
            cpu_time_limit_ms: args.cpu_time_limit_ms,
            wall_clock_timeout_ms: args.wall_clock_timeout_ms,
            inspect_port: None,
            inspect_brk: false,
            enable_source_maps: matches!(args.sourcemap, SourceMapMode::Inline),
        };

        let registry = Arc::new(functions::registry::FunctionRegistry::new(
            shutdown.clone(),
            default_config,
        ));

        // Spawn signal handler
        let shutdown_signal = shutdown.clone();
        tokio::spawn(edge_server::graceful::wait_for_shutdown_signal(shutdown_signal));

        // Build admin listener config
        let admin_addr: SocketAddr =
            format!("{}:{}", args.admin_host, args.admin_port).parse()?;
        let admin_tls = match (&args.admin_tls_cert, &args.admin_tls_key) {
            (Some(cert), Some(key)) => Some(edge_server::TlsConfig {
                cert_path: cert.clone(),
                key_path: key.clone(),
            }),
            _ => None,
        };

        // Build ingress listener config
        let ingress_type = match (&args.unix_socket, args.port) {
            (Some(path), _) => edge_server::IngressListenerType::Unix(path.clone()),
            (_, Some(port)) => {
                let addr: SocketAddr = format!("{}:{}", args.host, port).parse()?;
                edge_server::IngressListenerType::Tcp(addr)
            }
            (None, None) => {
                // Default: TCP port 8080
                let addr: SocketAddr = format!("{}:8080", args.host).parse()?;
                edge_server::IngressListenerType::Tcp(addr)
            }
        };

        let ingress_tls = match (&args.tls_cert, &args.tls_key, &args.unix_socket) {
            (Some(cert), Some(key), None) => Some(edge_server::TlsConfig {
                cert_path: cert.clone(),
                key_path: key.clone(),
            }),
            _ => None,
        };

        let config = edge_server::DualServerConfig {
            admin: edge_server::AdminListenerConfig {
                addr: admin_addr,
                api_key: args.api_key,
                tls: admin_tls,
            },
            ingress: edge_server::IngressListenerConfig {
                listener_type: ingress_type,
                tls: ingress_tls,
                rate_limit_rps: if args.rate_limit > 0 {
                    Some(args.rate_limit)
                } else {
                    None
                },
            },
            graceful_exit_deadline_secs: args.graceful_exit_timeout,
        };

        info!("starting deno-edge-runtime");

        // Run the dual-listener server (blocks until shutdown)
        edge_server::run_dual_server(config, registry.clone(), shutdown.clone()).await?;

        // Shutdown all functions
        registry.shutdown_all().await;

        info!("deno-edge-runtime stopped");
        Ok(())
    })
}
