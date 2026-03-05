use std::net::SocketAddr;
use std::sync::Arc;

use clap::Args;
use tokio_util::sync::CancellationToken;
use tracing::info;

use runtime_core::isolate::IsolateConfig;

#[derive(Args)]
pub struct StartArgs {
    /// IP address to bind
    #[arg(long, default_value = "0.0.0.0", env = "EDGE_RUNTIME_HOST")]
    host: String,

    /// Port to listen on
    #[arg(short, long, default_value_t = 9000, env = "EDGE_RUNTIME_PORT")]
    port: u16,

    /// TLS certificate file path
    #[arg(long, env = "EDGE_RUNTIME_TLS_CERT")]
    tls_cert: Option<String>,

    /// TLS private key file path
    #[arg(long, env = "EDGE_RUNTIME_TLS_KEY")]
    tls_key: Option<String>,

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
}

pub fn run(args: StartArgs) -> Result<(), anyhow::Error> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("edge-rt")
        .build()?;

    runtime.block_on(async {
        let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;
        let shutdown = CancellationToken::new();

        let default_config = IsolateConfig {
            max_heap_size_bytes: (args.max_heap_mib as usize) * 1024 * 1024,
            cpu_time_limit_ms: args.cpu_time_limit_ms,
            wall_clock_timeout_ms: args.wall_clock_timeout_ms,
        };

        let registry = Arc::new(functions::registry::FunctionRegistry::new(
            shutdown.clone(),
            default_config,
        ));

        // Spawn signal handler
        let shutdown_signal = shutdown.clone();
        tokio::spawn(edge_server::graceful::wait_for_shutdown_signal(shutdown_signal));

        let tls = match (args.tls_cert, args.tls_key) {
            (Some(cert), Some(key)) => Some(edge_server::TlsConfig {
                cert_path: cert,
                key_path: key,
            }),
            _ => None,
        };

        let config = edge_server::ServerConfig {
            addr,
            tls,
            rate_limit_rps: if args.rate_limit > 0 {
                Some(args.rate_limit)
            } else {
                None
            },
            graceful_exit_deadline_secs: args.graceful_exit_timeout,
        };

        info!("starting deno-edge-runtime on {}", addr);

        // Run the server (blocks until shutdown)
        edge_server::run_server(config, registry.clone(), shutdown.clone()).await?;

        // Shutdown all functions
        registry.shutdown_all().await;

        info!("deno-edge-runtime stopped");
        Ok(())
    })
}
