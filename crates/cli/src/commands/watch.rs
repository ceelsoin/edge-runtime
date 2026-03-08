use std::io::{Read, Write};
use std::net::SocketAddr;
use std::net::TcpStream;
#[cfg(unix)]
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use clap::Args;
use deno_ast::{EmitOptions, TranspileOptions};
use deno_graph::ast::CapturingModuleAnalyzer;
use deno_graph::source::{LoadError, LoadOptions, LoadResponse, Loader};
use deno_graph::{BuildOptions, GraphKind, ModuleGraph};
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};
use url::Url;

use runtime_core::isolate::{IsolateConfig, OutgoingProxyConfig};

use super::embedded_assert;

const DEFAULT_INSPECT_HOST: &str = "127.0.0.1";
const DEFAULT_INSPECT_PORT: u16 = 9229;

#[derive(Args)]
pub struct WatchArgs {
    /// Directory to watch (defaults to current directory)
    #[arg(default_value = ".", long)]
    path: String,

    /// IP address to bind
    #[arg(long, default_value = "0.0.0.0", env = "EDGE_RUNTIME_HOST")]
    host: String,

    /// Port to listen on
    #[arg(short, long, default_value_t = 9000, env = "EDGE_RUNTIME_PORT")]
    port: u16,

    /// Watch interval in milliseconds (debounce for file changes)
    #[arg(long, default_value_t = 1000)]
    interval: u64,

    /// Default max heap size per isolate in MiB (0 = unlimited)
    #[arg(long, default_value_t = 128, env = "EDGE_RUNTIME_MAX_HEAP_MIB")]
    max_heap_mib: u64,

    /// Default CPU time limit per request in ms (0 = unlimited)
    #[arg(long, default_value_t = 50000, env = "EDGE_RUNTIME_CPU_TIME_LIMIT_MS")]
    cpu_time_limit_ms: u64,

    /// Default wall clock timeout per request in ms (0 = unlimited)
    #[arg(
        long,
        default_value_t = 60000,
        env = "EDGE_RUNTIME_WALL_CLOCK_TIMEOUT_MS"
    )]
    wall_clock_timeout_ms: u64,

    /// Print user function `console.*` logs to runtime stdout.
    /// If disabled, logs are captured only by the internal isolate collector.
    #[arg(long, default_value_t = true, env = "EDGE_RUNTIME_PRINT_ISOLATE_LOGS")]
    print_isolate_logs: bool,

    /// Default VFS total writable quota in bytes per isolate.
    #[arg(
        long,
        default_value_t = 10 * 1024 * 1024,
        env = "EDGE_RUNTIME_VFS_TOTAL_QUOTA_BYTES"
    )]
    vfs_total_quota_bytes: usize,

    /// Default VFS max writable file size in bytes per isolate.
    #[arg(
        long,
        default_value_t = 5 * 1024 * 1024,
        env = "EDGE_RUNTIME_VFS_MAX_FILE_BYTES"
    )]
    vfs_max_file_bytes: usize,

    /// DNS-over-HTTPS resolver endpoint used by node:dns compatibility layer.
    #[arg(
        long,
        default_value = "https://1.1.1.1/dns-query",
        env = "EDGE_RUNTIME_DNS_DOH_ENDPOINT"
    )]
    dns_doh_endpoint: String,

    /// Maximum DNS answers returned per query by node:dns compatibility layer.
    #[arg(long, default_value_t = 16, env = "EDGE_RUNTIME_DNS_MAX_ANSWERS")]
    dns_max_answers: usize,

    /// DNS resolver timeout in milliseconds for node:dns compatibility layer.
    #[arg(long, default_value_t = 2000, env = "EDGE_RUNTIME_DNS_TIMEOUT_MS")]
    dns_timeout_ms: u64,

    /// Default node:zlib max output length in bytes (hard-ceiling enforced by runtime).
    #[arg(
        long,
        default_value_t = 16 * 1024 * 1024,
        env = "EDGE_RUNTIME_ZLIB_MAX_OUTPUT_LENGTH"
    )]
    zlib_max_output_length: usize,

    /// Default node:zlib max input length in bytes (hard-ceiling enforced by runtime).
    #[arg(
        long,
        default_value_t = 8 * 1024 * 1024,
        env = "EDGE_RUNTIME_ZLIB_MAX_INPUT_LENGTH"
    )]
    zlib_max_input_length: usize,

    /// Default node:zlib operation timeout in milliseconds.
    #[arg(
        long,
        default_value_t = 250,
        env = "EDGE_RUNTIME_ZLIB_OPERATION_TIMEOUT_MS"
    )]
    zlib_operation_timeout_ms: u64,

    /// Maximum outbound network requests per execution (0 = unlimited).
    #[arg(
        long,
        default_value_t = 0,
        env = "EDGE_RUNTIME_EGRESS_MAX_REQUESTS_PER_EXECUTION"
    )]
    egress_max_requests_per_execution: usize,

    /// Outgoing HTTP proxy URL (eg. http://proxy.local:8080, socks5://proxy.local:1080)
    #[arg(long, env = "EDGE_RUNTIME_HTTP_OUTGOING_PROXY")]
    http_outgoing_proxy: Option<String>,

    /// Outgoing HTTPS proxy URL (eg. http://proxy.local:8080, socks5://proxy.local:1080)
    #[arg(long, env = "EDGE_RUNTIME_HTTPS_OUTGOING_PROXY")]
    https_outgoing_proxy: Option<String>,

    /// Outgoing TCP proxy endpoint (host:port or tcp://host:port)
    #[arg(long, env = "EDGE_RUNTIME_TCP_OUTGOING_PROXY")]
    tcp_outgoing_proxy: Option<String>,

    /// Bypass list for HTTP proxy (comma-separated hosts/domains)
    #[arg(long, value_delimiter = ',', env = "EDGE_RUNTIME_HTTP_NO_PROXY")]
    http_no_proxy: Vec<String>,

    /// Bypass list for HTTPS proxy (comma-separated hosts/domains)
    #[arg(long, value_delimiter = ',', env = "EDGE_RUNTIME_HTTPS_NO_PROXY")]
    https_no_proxy: Vec<String>,

    /// Bypass list for TCP proxy (comma-separated hosts/domains)
    #[arg(long, value_delimiter = ',', env = "EDGE_RUNTIME_TCP_NO_PROXY")]
    tcp_no_proxy: Vec<String>,

    /// Enable V8 inspector protocol in watch mode (optional base port, default: 9229)
    ///
    /// When multiple functions are loaded, ports are assigned sequentially:
    /// base, base+1, base+2, ... in deployment order.
    #[arg(long, value_name = "PORT", num_args = 0..=1, default_missing_value = "9229")]
    inspect: Option<u16>,

    /// Wait for debugger attach and break on first statement (requires --inspect)
    #[arg(long, default_value_t = false)]
    inspect_brk: bool,

    /// Allow inspector to bind on all interfaces (0.0.0.0). Unsafe for production.
    #[arg(long, default_value_t = false)]
    inspect_allow_remote: bool,
}

/// A simple file-system loader for deno_graph.
struct FileLoader;

impl Loader for FileLoader {
    fn load(
        &self,
        specifier: &deno_graph::ModuleSpecifier,
        _options: LoadOptions,
    ) -> deno_graph::source::LoadFuture {
        let specifier = specifier.clone();
        Box::pin(async move {
            if specifier.scheme() == "edge" || specifier.scheme() == "ext" {
                if let Some(content) = load_edge_assert_module(&specifier)? {
                    return Ok(Some(LoadResponse::Module {
                        content: content.into(),
                        specifier,
                        maybe_headers: None,
                        mtime: None,
                    }));
                }
            }

            if specifier.scheme() != "file" {
                return Ok(None);
            }

            let path = specifier.to_file_path().map_err(|()| {
                LoadError::Other(Arc::new(deno_error::JsErrorBox::generic(format!(
                    "invalid file URL: {specifier}"
                ))))
            })?;

            let content = std::fs::read(&path).map_err(|e| {
                LoadError::Other(Arc::new(deno_error::JsErrorBox::generic(format!(
                    "failed to read '{}': {e}",
                    path.display()
                ))))
            })?;

            let content = rewrite_edge_assert_imports(content)?;

            Ok(Some(LoadResponse::Module {
                content: content.into(),
                specifier,
                maybe_headers: None,
                mtime: None,
            }))
        })
    }
}

fn rewrite_edge_assert_imports(content: Vec<u8>) -> Result<Vec<u8>, LoadError> {
    Ok(embedded_assert::rewrite_edge_assert_imports(content))
}

fn load_edge_assert_module(
    specifier: &deno_graph::ModuleSpecifier,
) -> Result<Option<Vec<u8>>, LoadError> {
    embedded_assert::load_module_bytes(specifier)
}

pub fn run(args: WatchArgs) -> Result<(), anyhow::Error> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("edge-rt-watch")
        .build()?;

    runtime.block_on(async {
        let path = Path::new(&args.path);

        if !path.exists() {
            return Err(anyhow::anyhow!("path '{}' does not exist", args.path));
        }

        let effective_inspect = resolve_watch_inspect_config(&args);

        if args.inspect.is_none() {
            if let Some(base_port) = effective_inspect.port {
                info!(
                    "auto-detected inspector base port {} from environment",
                    base_port
                );
            }
        }

        if args.inspect_allow_remote && effective_inspect.port.is_none() {
            return Err(anyhow::anyhow!(
                "--inspect-allow-remote requires --inspect"
            ));
        }

        let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;
        let shutdown = CancellationToken::new();

        let default_config = build_watch_default_config(
            &args,
            effective_inspect.inspect_brk,
            effective_inspect.inspect_allow_remote,
        );

        if let Some(base_port) = effective_inspect.port {
            warn!(
                "V8 inspector is enabled in watch mode on base port {}. Do not use this in production.",
                base_port
            );
            if effective_inspect.inspect_allow_remote {
                warn!(
                    "Inspector remote access is enabled (--inspect-allow-remote). Debug endpoints are exposed on all interfaces."
                );
            }
        }

        let registry = Arc::new(functions::registry::FunctionRegistry::new_with_pool(
            shutdown.clone(),
            default_config.clone(),
            build_watch_pool_config(&args),
            functions::types::PoolLimits::default(),
        ));

        crate::telemetry::spawn_isolate_log_exporter(
            shutdown.clone(),
            args.print_isolate_logs,
        );

        // Spawn signal handler for graceful shutdown
        let shutdown_signal = shutdown.clone();
        tokio::spawn(edge_server::graceful::wait_for_shutdown_signal(shutdown_signal));

        let server_config = edge_server::ServerConfig {
            addr,
            tls: None,
            rate_limit_rps: None,
            // Watch mode favors fast feedback and instant cancellation.
            graceful_exit_deadline_secs: 0,
            body_limits: edge_server::BodyLimitsConfig::default(),
            max_connections: 10_000,
        };

        info!("starting edge runtime in watch mode on {}", addr);
        info!("watching '{}' for TypeScript/JavaScript files", path.display());

        // Spawn the server
        let registry_clone = registry.clone();
        let shutdown_clone = shutdown.clone();
        let server_handle = tokio::spawn(async move {
            if let Err(e) = edge_server::run_server(server_config, registry_clone.clone(), shutdown_clone).await {
                tracing::error!("server error: {}", e);
            }
        });

        // Setup file watcher channel
        let (tx, mut rx) = mpsc::unbounded_channel();
        let watch_path = path.to_path_buf();

        std::thread::spawn(move || {
            use notify::{Watcher, RecursiveMode};

            let mut watcher = match notify::recommended_watcher(move |_res: notify::Result<_>| {
                let _ = tx.send(());
            }) {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("Failed to create watcher: {}", e);
                    return;
                }
            };

            if let Err(e) = watcher.watch(&watch_path, RecursiveMode::Recursive) {
                eprintln!("Failed to watch directory: {}", e);
                return;
            }

            // Keep watcher alive
            loop {
                std::thread::sleep(Duration::from_secs(1));
            }
        });

        // Initial load of functions
        let inspector_ports =
            load_and_deploy_functions(path, &registry, &default_config, effective_inspect.port)
                .await?;
        notify_vscode_auto_attach(
            effective_inspect.vscode_options.as_ref(),
            &inspector_ports,
            &args.path,
        );

        let mut last_update = tokio::time::Instant::now();
        let debounce_duration = Duration::from_millis(args.interval);

        tokio::select! {
            _ = server_handle => {
                info!("server exited");
            }
            _ = async {
                loop {
                    if let Some(_) = rx.recv().await {
                        let now = tokio::time::Instant::now();
                        if now.duration_since(last_update) >= debounce_duration {
                            println!("\n{}", "─".repeat(80));
                            println!("🔄 Changes detected, reloading...");
                            match load_and_deploy_functions(path, &registry, &default_config, effective_inspect.port).await {
                                Ok(inspector_ports) => {
                                    notify_vscode_auto_attach(
                                        effective_inspect.vscode_options.as_ref(),
                                        &inspector_ports,
                                        &args.path,
                                    );
                                }
                                Err(e) => {
                                    eprintln!("❌ Error loading functions: {}", e);
                                }
                            }
                            last_update = now;
                        }
                    }
                }
            } => {}
        }

        // In watch mode we prefer immediate cancellation over graceful draining.
        // Try a short shutdown window for isolates, then continue process exit.
        if tokio::time::timeout(Duration::from_millis(200), registry.shutdown_all())
            .await
            .is_err()
        {
            tracing::warn!("watch shutdown timeout reached; forcing immediate exit");
        }

        info!("edge runtime watch mode stopped");
        Ok(())
    })
}

#[derive(Debug, Clone)]
struct EffectiveInspectConfig {
    port: Option<u16>,
    inspect_brk: bool,
    inspect_allow_remote: bool,
    vscode_options: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NodeInspectOptions {
    enabled: bool,
    host: Option<String>,
    port: Option<u16>,
    brk: bool,
}

fn resolve_watch_inspect_config(args: &WatchArgs) -> EffectiveInspectConfig {
    let node_options = std::env::var("NODE_OPTIONS")
        .ok()
        .map(|raw| parse_node_options(&raw))
        .unwrap_or(NodeInspectOptions {
            enabled: false,
            host: None,
            port: None,
            brk: false,
        });

    let mut port = args.inspect;
    if port.is_none() {
        port = detect_inspect_port_from_env();
    }
    if port.is_none() && node_options.enabled {
        port = Some(node_options.port.unwrap_or(DEFAULT_INSPECT_PORT));
    }

    let vscode_options_raw = std::env::var("VSCODE_INSPECTOR_OPTIONS").ok();
    let inspect_requested_by_vscode = vscode_options_raw.is_some();
    if port.is_none() && inspect_requested_by_vscode {
        // VS Code auto-attach may only signal intent via VSCODE_INSPECTOR_OPTIONS.
        port = Some(DEFAULT_INSPECT_PORT);
    }

    let vscode_wait_for_debugger = vscode_options_raw
        .as_deref()
        .map(parse_wait_for_debugger_from_vscode_options)
        .unwrap_or(false);

    let inspect_allow_remote = args.inspect_allow_remote
        || node_options
            .host
            .as_deref()
            .map(|h| !is_loopback_host(h))
            .unwrap_or(false);

    EffectiveInspectConfig {
        port,
        inspect_brk: args.inspect_brk || node_options.brk || vscode_wait_for_debugger,
        inspect_allow_remote,
        vscode_options: vscode_options_raw
            .as_deref()
            .and_then(parse_vscode_inspector_options_json),
    }
}

fn detect_inspect_port_from_env() -> Option<u16> {
    for key in [
        "EDGE_RUNTIME_INSPECT_PORT",
        "VSCODE_INSPECTOR_PORT",
        "VSCODE_JS_DEBUG_PORT",
        "JS_DEBUG_PORT",
        "INSPECT_PORT",
    ] {
        if let Ok(value) = std::env::var(key) {
            if let Some(port) = parse_inspect_port_value(&value) {
                return Some(port);
            }
        }
    }

    if let Ok(options) = std::env::var("VSCODE_INSPECTOR_OPTIONS") {
        if let Some(port) = parse_inspect_port_from_vscode_options(&options) {
            return Some(port);
        }
    }

    None
}

fn parse_inspect_port_value(raw: &str) -> Option<u16> {
    let parsed = raw.trim().parse::<u16>().ok()?;
    if parsed == 0 {
        return None;
    }
    Some(parsed)
}

fn parse_node_options(raw: &str) -> NodeInspectOptions {
    let mut parsed = NodeInspectOptions {
        enabled: false,
        host: None,
        port: None,
        brk: false,
    };

    for token in raw.split_whitespace() {
        if token == "--inspect" {
            parsed.enabled = true;
            continue;
        }

        if token == "--inspect-brk" {
            parsed.enabled = true;
            parsed.brk = true;
            continue;
        }

        if let Some(value) = token.strip_prefix("--inspect=") {
            parsed.enabled = true;
            if let Some((host, port)) = parse_inspect_endpoint(value) {
                if host.is_some() {
                    parsed.host = host;
                }
                if port.is_some() {
                    parsed.port = port;
                }
            }
            continue;
        }

        if let Some(value) = token.strip_prefix("--inspect-brk=") {
            parsed.enabled = true;
            parsed.brk = true;
            if let Some((host, port)) = parse_inspect_endpoint(value) {
                if host.is_some() {
                    parsed.host = host;
                }
                if port.is_some() {
                    parsed.port = port;
                }
            }
            continue;
        }

        if let Some(value) = token.strip_prefix("--inspect-port=") {
            if let Some(port) = parse_inspect_port_value(value) {
                parsed.port = Some(port);
            }
        }
    }

    parsed
}

fn parse_inspect_endpoint(value: &str) -> Option<(Option<String>, Option<u16>)> {
    let value = value.trim();
    if value.is_empty() {
        return Some((
            Some(DEFAULT_INSPECT_HOST.to_string()),
            Some(DEFAULT_INSPECT_PORT),
        ));
    }

    if let Some(port) = parse_inspect_port_value(value) {
        return Some((Some(DEFAULT_INSPECT_HOST.to_string()), Some(port)));
    }

    if let Some(host) = value.strip_prefix('[').and_then(|v| v.split_once("]:")) {
        let port = parse_inspect_port_value(host.1).unwrap_or(DEFAULT_INSPECT_PORT);
        return Some((Some(host.0.to_string()), Some(port)));
    }

    if let Some((host, port_raw)) = value.rsplit_once(':') {
        if !host.is_empty() {
            if let Some(port) = parse_inspect_port_value(port_raw) {
                return Some((Some(host.to_string()), Some(port)));
            }
        }
    }

    Some((Some(value.to_string()), Some(DEFAULT_INSPECT_PORT)))
}

fn is_loopback_host(host: &str) -> bool {
    let host = host.trim_matches(['[', ']']);
    matches!(host, "127.0.0.1" | "localhost" | "::1")
}

fn parse_inspect_port_from_vscode_options(raw_json: &str) -> Option<u16> {
    let parsed = parse_vscode_inspector_options_json(raw_json)?;

    for key in ["port", "inspectorPort", "inspector_port"] {
        let maybe_port = parsed.get(key).and_then(|v| {
            if let Some(num) = v.as_u64() {
                return u16::try_from(num).ok().filter(|p| *p > 0);
            }
            if let Some(s) = v.as_str() {
                return parse_inspect_port_value(s);
            }
            None
        });
        if maybe_port.is_some() {
            return maybe_port;
        }
    }

    None
}

fn parse_wait_for_debugger_from_vscode_options(raw_json: &str) -> bool {
    let Some(parsed) = parse_vscode_inspector_options_json(raw_json) else {
        return false;
    };

    parsed
        .get("waitForDebugger")
        .map(|value| match value {
            Value::Bool(flag) => *flag,
            Value::String(s) => !s.trim().is_empty(),
            _ => false,
        })
        .unwrap_or(false)
}

fn parse_vscode_inspector_options_json(raw: &str) -> Option<Value> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
        return Some(parsed);
    }

    // JS Debug Terminal may prefix payload with markers like ":::{...}".
    let start = trimmed.find('{')?;
    let candidate = &trimmed[start..];
    if let Ok(parsed) = serde_json::from_str::<Value>(candidate) {
        return Some(parsed);
    }

    let end = candidate.rfind('}')?;
    serde_json::from_str::<Value>(&candidate[..=end]).ok()
}

async fn load_and_deploy_functions(
    path: &Path,
    registry: &Arc<functions::registry::FunctionRegistry>,
    default_config: &IsolateConfig,
    inspect_base_port: Option<u16>,
) -> anyhow::Result<Vec<u16>> {
    info!("scanning {}", path.display());

    let ts_js_pattern = regex::Regex::new(r"\.(ts|js)$")?;

    let mut deployed = 0;
    let mut skipped = 0;
    let mut deployed_inspector_ports = Vec::new();

    let mut source_files: Vec<std::path::PathBuf> = walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .map(|e| e.path().to_path_buf())
        .collect();
    source_files.sort();

    let mut inspect_index: u16 = 0;
    for file_path in source_files.iter() {
        // Skip node_modules, dist, build, etc.
        if file_path.components().any(|c| {
            let s = c.as_os_str().to_string_lossy();
            matches!(
                s.as_ref(),
                "node_modules" | "dist" | "build" | ".next" | ".deno" | "target"
            )
        }) {
            continue;
        }

        if !ts_js_pattern.is_match(file_path.to_string_lossy().as_ref()) {
            continue;
        }

        // Generate function name from path.
        // If watch target is a single file, strip_prefix(path) becomes empty,
        // so we fallback to the filename to keep stable names like "hello".
        let relative_path = if path.is_file() {
            file_path
                .file_name()
                .map(Path::new)
                .unwrap_or(file_path.as_path())
        } else {
            file_path.strip_prefix(path).unwrap_or(file_path.as_path())
        };
        let func_name = path_to_function_name(relative_path);

        let inspect_port = if let Some(base) = inspect_base_port {
            let port = base
                .checked_add(inspect_index)
                .ok_or_else(|| anyhow::anyhow!("inspector port overflow for '{}'", func_name))?;
            inspect_index = inspect_index.saturating_add(1);
            Some(port)
        } else {
            None
        };

        let function_config = IsolateConfig {
            max_heap_size_bytes: default_config.max_heap_size_bytes,
            cpu_time_limit_ms: default_config.cpu_time_limit_ms,
            wall_clock_timeout_ms: default_config.wall_clock_timeout_ms,
            egress_max_requests_per_execution: default_config.egress_max_requests_per_execution,
            inspect_port,
            inspect_brk: default_config.inspect_brk,
            inspect_allow_remote: default_config.inspect_allow_remote,
            enable_source_maps: default_config.enable_source_maps,
            ssrf_config: default_config.ssrf_config.clone(),
            print_isolate_logs: default_config.print_isolate_logs,
            vfs_total_quota_bytes: default_config.vfs_total_quota_bytes,
            vfs_max_file_bytes: default_config.vfs_max_file_bytes,
            dns_doh_endpoint: default_config.dns_doh_endpoint.clone(),
            dns_max_answers: default_config.dns_max_answers,
            dns_timeout_ms: default_config.dns_timeout_ms,
            zlib_max_output_length: default_config.zlib_max_output_length,
            zlib_max_input_length: default_config.zlib_max_input_length,
            zlib_operation_timeout_ms: default_config.zlib_operation_timeout_ms,
            context_pool_enabled: default_config.context_pool_enabled,
            max_contexts_per_isolate: default_config.max_contexts_per_isolate,
            max_active_requests_per_context: default_config.max_active_requests_per_context,
        };

        match bundle_file(file_path).await {
            Ok(eszip_bytes) => {
                let bytes = Bytes::from(eszip_bytes);

                // Try to deploy (or update if exists)
                match registry
                    .deploy(
                        func_name.clone(),
                        bytes.clone(),
                        Some(function_config.clone()),
                        None,
                    )
                    .await
                {
                    Ok(_info) => {
                        println!("✅ Deployed: {} ({} bytes)", func_name, bytes.len());
                        if let Some(port) = inspect_port {
                            deployed_inspector_ports.push(port);
                            let host = if function_config.inspect_allow_remote {
                                "0.0.0.0"
                            } else {
                                "127.0.0.1"
                            };
                            println!("   └─ inspector: ws://{}:{}/ws", host, port);
                        }
                        deployed += 1;
                    }
                    Err(e) if e.to_string().contains("already exists") => {
                        // Try to update instead
                        match registry
                            .update(
                                &func_name,
                                bytes.clone(),
                                Some(function_config.clone()),
                                None,
                            )
                            .await
                        {
                            Ok(_) => {
                                println!("🔄 Updated: {}", func_name);
                                if let Some(port) = inspect_port {
                                    deployed_inspector_ports.push(port);
                                    let host = if function_config.inspect_allow_remote {
                                        "0.0.0.0"
                                    } else {
                                        "127.0.0.1"
                                    };
                                    println!("   └─ inspector: ws://{}:{}/ws", host, port);
                                }
                                deployed += 1;
                            }
                            Err(e) => {
                                eprintln!("❌ Failed to update '{}': {}", func_name, e);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("❌ Failed to deploy '{}': {}", func_name, e);
                    }
                }
            }
            Err(e) => {
                eprintln!("❌ Failed to bundle '{}': {}", file_path.display(), e);
                skipped += 1;
            }
        }
    }

    println!("\n{}", "─".repeat(80));
    println!("📊 Summary: {} deployed, {} skipped", deployed, skipped);

    Ok(deployed_inspector_ports)
}

fn notify_vscode_auto_attach(
    vscode_options: Option<&Value>,
    inspector_ports: &[u16],
    script_name: &str,
) {
    let Some(options) = vscode_options else {
        return;
    };

    let Some(ipc_path) = options
        .get("inspectorIpc")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
    else {
        return;
    };

    for port in inspector_ports {
        let Some(inspector_url) = fetch_inspector_websocket_url(*port) else {
            tracing::debug!(
                "failed to resolve inspector URL from /json/list for port {}; skipping vscode auto-attach notify",
                port
            );
            continue;
        };

        if let Err(err) =
            notify_vscode_inspector_ipc(ipc_path, options, &inspector_url, script_name)
        {
            tracing::warn!(
                "failed to notify VS Code inspector IPC '{}': {}",
                ipc_path,
                err
            );
        }
    }
}

fn fetch_inspector_websocket_url(port: u16) -> Option<String> {
    let addr = format!("127.0.0.1:{}", port);
    let mut stream = TcpStream::connect(addr).ok()?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));

    let request = format!(
        "GET /json/list HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n",
        port
    );
    stream.write_all(request.as_bytes()).ok()?;

    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;
    let (_, body) = response.split_once("\r\n\r\n")?;

    let parsed: Value = serde_json::from_str(body).ok()?;
    parsed
        .as_array()?
        .first()?
        .get("webSocketDebuggerUrl")?
        .as_str()
        .map(|s| s.to_string())
}

fn notify_vscode_inspector_ipc(
    ipc_path: &str,
    options: &Value,
    inspector_url: &str,
    script_name: &str,
) -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        let mut stream = UnixStream::connect(ipc_path)?;
        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));

        let payload = serde_json::json!({
            "ipcAddress": ipc_path,
            "pid": std::process::id().to_string(),
            "telemetry": {
                "cwd": std::env::current_dir()
                    .ok()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default(),
                "processId": std::process::id(),
                "nodeVersion": format!("thunder/{}", env!("CARGO_PKG_VERSION")),
                "architecture": std::env::consts::ARCH,
            },
            "scriptName": script_name,
            "inspectorURL": inspector_url,
            "waitForDebugger": true,
            "ownId": format!("thunder-{}", std::process::id()),
            "openerId": options.get("openerId").and_then(|v| v.as_str()).unwrap_or(""),
        });

        let mut bytes = serde_json::to_vec(&payload)?;
        bytes.push(0);
        stream.write_all(&bytes)?;

        // js-debug replies with one byte status where 0 indicates success.
        let mut status = [0_u8; 1];
        if stream.read_exact(&mut status).is_ok() && status[0] != 0 {
            anyhow::bail!(
                "VS Code inspector IPC returned non-zero status: {}",
                status[0]
            );
        }

        return Ok(());
    }

    #[cfg(not(unix))]
    {
        let _ = (ipc_path, options, inspector_url, script_name);
        anyhow::bail!(
            "VS Code inspector IPC auto-attach is currently supported only on unix platforms"
        );
    }
}

async fn bundle_file(file_path: &Path) -> anyhow::Result<Vec<u8>> {
    let entrypoint = file_path
        .canonicalize()
        .map_err(|e| anyhow::anyhow!("cannot resolve '{}': {e}", file_path.display()))?;

    let root_url = Url::from_file_path(&entrypoint)
        .map_err(|()| anyhow::anyhow!("cannot convert path to URL: {}", entrypoint.display()))?;

    // Build module graph
    let loader = FileLoader;
    let analyzer = CapturingModuleAnalyzer::default();

    let mut graph = ModuleGraph::new(GraphKind::CodeOnly);
    graph
        .build(
            vec![root_url.clone()],
            vec![], // referrer imports
            &loader,
            BuildOptions {
                module_analyzer: &analyzer,
                ..Default::default()
            },
        )
        .await;

    graph
        .valid()
        .map_err(|e| anyhow::anyhow!("module graph error: {e}"))?;

    // Create eszip from graph
    let eszip = eszip::EszipV2::from_graph(eszip::FromGraphOptions {
        graph,
        parser: analyzer.as_capturing_parser(),
        module_kind_resolver: Default::default(),
        transpile_options: TranspileOptions::default(),
        emit_options: EmitOptions::default(),
        relative_file_base: None,
        npm_packages: None,
        npm_snapshot: Default::default(),
    })?;

    let eszip_bytes = eszip.into_bytes();

    // Package the bundle
    let pkg = functions::types::BundlePackage::eszip_only(eszip_bytes);
    let bundle_data = bincode::serialize(&pkg)?;

    Ok(bundle_data)
}

fn path_to_function_name(path: &Path) -> String {
    let path_str = path.to_string_lossy();

    // Remove file extension
    let path_str = if path_str.ends_with(".ts") {
        &path_str[..path_str.len() - 3]
    } else if path_str.ends_with(".js") {
        &path_str[..path_str.len() - 3]
    } else {
        &path_str
    };

    // Split by path separator
    let parts: Vec<&str> = path_str.split('/').filter(|p| !p.is_empty()).collect();

    if parts.is_empty() {
        return "unknown".to_string();
    }

    // If we have at least 2 parts and the last part equals the second-to-last part,
    // it means the directory and file have the same name (e.g., hello/hello.ts)
    // In that case, take only the last part to avoid duplication
    if parts.len() >= 2 && parts[parts.len() - 1] == parts[parts.len() - 2] {
        // Remove the duplicate and use only part of the path
        let relevant_parts = &parts[0..parts.len() - 1];
        relevant_parts.join("-")
    } else {
        // Use all parts, joining with dashes
        parts.join("-")
    }
}

fn build_watch_default_config(
    args: &WatchArgs,
    inspect_brk: bool,
    inspect_allow_remote: bool,
) -> IsolateConfig {
    IsolateConfig {
        max_heap_size_bytes: (args.max_heap_mib as usize) * 1024 * 1024,
        cpu_time_limit_ms: args.cpu_time_limit_ms,
        wall_clock_timeout_ms: args.wall_clock_timeout_ms,
        inspect_port: None,
        inspect_brk,
        inspect_allow_remote,
        enable_source_maps: true,
        // Watch mode is local-dev oriented: do not enforce SSRF network denylist.
        ssrf_config: runtime_core::ssrf::SsrfConfig::disabled(),
        print_isolate_logs: args.print_isolate_logs,
        vfs_total_quota_bytes: args.vfs_total_quota_bytes,
        vfs_max_file_bytes: args.vfs_max_file_bytes,
        dns_doh_endpoint: args.dns_doh_endpoint.clone(),
        dns_max_answers: args.dns_max_answers,
        dns_timeout_ms: args.dns_timeout_ms,
        zlib_max_output_length: args.zlib_max_output_length,
        zlib_max_input_length: args.zlib_max_input_length,
        zlib_operation_timeout_ms: args.zlib_operation_timeout_ms,
        egress_max_requests_per_execution: args.egress_max_requests_per_execution,
        context_pool_enabled: false,
        max_contexts_per_isolate: 8,
        max_active_requests_per_context: 1,
    }
}

fn build_watch_pool_config(args: &WatchArgs) -> functions::registry::PoolRuntimeConfig {
    functions::registry::PoolRuntimeConfig {
        enabled: false,
        global_max_isolates: 1024,
        min_free_memory_mib: 256,
        outgoing_proxy: OutgoingProxyConfig {
            http_proxy: args.http_outgoing_proxy.clone(),
            https_proxy: args.https_outgoing_proxy.clone(),
            tcp_proxy: args.tcp_outgoing_proxy.clone(),
            http_no_proxy: args.http_no_proxy.clone(),
            https_no_proxy: args.https_no_proxy.clone(),
            tcp_no_proxy: args.tcp_no_proxy.clone(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watch_default_config_disables_ssrf_protection() {
        let args = WatchArgs {
            path: ".".to_string(),
            host: "0.0.0.0".to_string(),
            port: 9000,
            interval: 1000,
            max_heap_mib: 128,
            cpu_time_limit_ms: 50_000,
            wall_clock_timeout_ms: 60_000,
            inspect: None,
            inspect_brk: false,
            inspect_allow_remote: false,
            print_isolate_logs: true,
            vfs_total_quota_bytes: 10 * 1024 * 1024,
            vfs_max_file_bytes: 5 * 1024 * 1024,
            dns_doh_endpoint: "https://1.1.1.1/dns-query".to_string(),
            dns_max_answers: 16,
            dns_timeout_ms: 2000,
            zlib_max_output_length: 16 * 1024 * 1024,
            zlib_max_input_length: 8 * 1024 * 1024,
            zlib_operation_timeout_ms: 250,
            egress_max_requests_per_execution: 0,
            http_outgoing_proxy: None,
            https_outgoing_proxy: None,
            tcp_outgoing_proxy: None,
            http_no_proxy: vec![],
            https_no_proxy: vec![],
            tcp_no_proxy: vec![],
        };

        let cfg = build_watch_default_config(&args, false, false);
        assert!(
            !cfg.ssrf_config.enabled,
            "watch mode must allow all network by default"
        );
        assert!(cfg.ssrf_config.allow_private_subnets.is_empty());
    }

    #[test]
    fn parse_inspect_port_value_accepts_valid_u16() {
        assert_eq!(parse_inspect_port_value("9229"), Some(9229));
        assert_eq!(parse_inspect_port_value(" 9230 "), Some(9230));
    }

    #[test]
    fn parse_inspect_port_value_rejects_invalid_or_zero() {
        assert_eq!(parse_inspect_port_value("0"), None);
        assert_eq!(parse_inspect_port_value("-1"), None);
        assert_eq!(parse_inspect_port_value("not-a-port"), None);
    }

    #[test]
    fn parse_inspect_port_from_vscode_options_supports_common_keys() {
        assert_eq!(
            parse_inspect_port_from_vscode_options(r#"{"inspectorPort":9235}"#),
            Some(9235)
        );
        assert_eq!(
            parse_inspect_port_from_vscode_options(r#"{"port":9240}"#),
            Some(9240)
        );
        assert_eq!(
            parse_inspect_port_from_vscode_options(r#"{"inspector_port":9250}"#),
            Some(9250)
        );
        assert_eq!(
            parse_inspect_port_from_vscode_options(r#"{"port":"9255"}"#),
            Some(9255)
        );
        assert_eq!(
            parse_inspect_port_from_vscode_options(r#"{"inspectorIpc":"/tmp/node-cdp.sock"}"#),
            None
        );
        assert_eq!(
            parse_inspect_port_from_vscode_options(
                r#":::{"inspectorIpc":"/tmp/node-cdp.sock","port":"9260"}"#
            ),
            Some(9260)
        );
    }

    #[test]
    fn parse_wait_for_debugger_from_vscode_options_supports_bool_and_string() {
        assert!(parse_wait_for_debugger_from_vscode_options(
            r#"{"waitForDebugger":true}"#
        ));
        assert!(parse_wait_for_debugger_from_vscode_options(
            r#":::{"waitForDebugger":"1"}"#
        ));
        assert!(!parse_wait_for_debugger_from_vscode_options(
            r#"{"waitForDebugger":""}"#
        ));
        assert!(!parse_wait_for_debugger_from_vscode_options("not-json"));
    }

    #[test]
    fn parse_node_options_supports_inspect_and_inspect_brk() {
        let parsed = parse_node_options("--inspect --inspect-brk --inspect-port=9333");
        assert!(parsed.enabled);
        assert!(parsed.brk);
        assert_eq!(parsed.port, Some(9333));
    }

    #[test]
    fn parse_node_options_supports_host_port_endpoint() {
        let parsed = parse_node_options("--inspect=0.0.0.0:9444");
        assert!(parsed.enabled);
        assert_eq!(parsed.host.as_deref(), Some("0.0.0.0"));
        assert_eq!(parsed.port, Some(9444));
    }

    #[test]
    fn parse_inspect_endpoint_defaults_port_when_only_host() {
        let parsed = parse_inspect_endpoint("localhost").expect("endpoint parse");
        assert_eq!(parsed.0.as_deref(), Some("localhost"));
        assert_eq!(parsed.1, Some(DEFAULT_INSPECT_PORT));
    }

    #[test]
    fn is_loopback_host_handles_common_forms() {
        assert!(is_loopback_host("127.0.0.1"));
        assert!(is_loopback_host("localhost"));
        assert!(is_loopback_host("[::1]"));
        assert!(!is_loopback_host("0.0.0.0"));
    }

    #[cfg(unix)]
    #[test]
    fn notify_vscode_inspector_ipc_sends_json_payload_with_nul_terminator() {
        use std::os::unix::net::UnixListener;

        let socket_path = std::env::temp_dir().join(format!(
            "thunder-inspector-test-{}-{}.sock",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock before epoch")
                .as_nanos()
        ));

        let listener = UnixListener::bind(&socket_path).expect("bind unix listener");

        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept socket");
            let mut received = Vec::new();
            loop {
                let mut b = [0_u8; 1];
                stream.read_exact(&mut b).expect("read byte from socket");
                if b[0] == 0 {
                    break;
                }
                received.push(b[0]);
            }
            stream.write_all(&[0_u8]).expect("write success status");
            received
        });

        let options = serde_json::json!({"openerId":"from-test"});
        let inspector_url = "ws://127.0.0.1:9229/runtime";
        notify_vscode_inspector_ipc(
            socket_path.to_str().expect("utf8 socket path"),
            &options,
            inspector_url,
            "./examples/hello/hello.ts",
        )
        .expect("notify vscode inspector ipc");

        let payload = handle.join().expect("server thread join");
        std::fs::remove_file(&socket_path).expect("remove unix socket path");

        let payload: Value =
            serde_json::from_slice(&payload).expect("decode payload json sent to ipc socket");
        assert_eq!(
            payload.get("inspectorURL").and_then(|v| v.as_str()),
            Some(inspector_url)
        );
        assert_eq!(
            payload.get("openerId").and_then(|v| v.as_str()),
            Some("from-test")
        );
    }
}
