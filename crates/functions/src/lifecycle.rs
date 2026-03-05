use std::sync::atomic::Ordering;
use std::sync::Arc;

use anyhow::Error;
use bytes::Bytes;
use chrono::Utc;
use deno_core::{JsRuntime, PollEventLoopOptions, RuntimeOptions};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use runtime_core::extensions;
use runtime_core::isolate::{determine_root_specifier, IsolateConfig, IsolateHandle, IsolateRequest};
use runtime_core::module_loader::EszipModuleLoader;

use crate::handler;
use crate::types::*;

/// Create a FunctionEntry: parse eszip, boot isolate on a dedicated thread.
pub async fn create_function(
    name: String,
    eszip_bytes: Bytes,
    config: IsolateConfig,
    parent_shutdown: CancellationToken,
) -> Result<FunctionEntry, Error> {
    let now = Utc::now();
    let metrics = Arc::new(FunctionMetrics::default());

    // Parse the eszip bundle
    let reader = futures_util::io::BufReader::new(futures_util::io::Cursor::new(eszip_bytes.to_vec()));
    let (eszip, loader_fut) = eszip::EszipV2::parse(reader)
        .await
        .map_err(|e| anyhow::anyhow!("failed to parse eszip: {e}"))?;

    // Spawn the lazy loader future
    tokio::spawn(loader_fut);

    let eszip = Arc::new(eszip);
    let root_specifier = determine_root_specifier(&eszip)?;

    // Create the request channel
    let (request_tx, request_rx) = mpsc::unbounded_channel::<IsolateRequest>();

    // Build the IsolateHandle
    let shutdown = parent_shutdown.child_token();
    let handle = IsolateHandle {
        request_tx,
        shutdown: shutdown.clone(),
        id: uuid::Uuid::new_v4(),
    };

    // Spawn the isolate on a dedicated thread (JsRuntime is !Send)
    let isolate_name = name.clone();
    let isolate_config = config.clone();
    let isolate_metrics = metrics.clone();

    std::thread::Builder::new()
        .name(format!("fn-{}", name))
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to build tokio runtime for isolate");

            let local = tokio::task::LocalSet::new();
            local.block_on(&rt, async move {
                match run_isolate(
                    isolate_name.clone(),
                    eszip,
                    root_specifier,
                    isolate_config,
                    request_rx,
                    shutdown,
                    isolate_metrics,
                )
                .await
                {
                    Ok(()) => info!("isolate '{}' exited cleanly", isolate_name),
                    Err(e) => error!("isolate '{}' exited with error: {}", isolate_name, e),
                }
            });
        })
        .map_err(|e| anyhow::anyhow!("failed to spawn isolate thread: {e}"))?;

    Ok(FunctionEntry {
        name,
        eszip_bytes,
        isolate_handle: Some(handle),
        status: FunctionStatus::Running,
        config,
        metrics,
        created_at: now,
        updated_at: now,
        last_error: None,
    })
}

/// The long-running isolate event loop.
async fn run_isolate(
    name: String,
    eszip: Arc<eszip::EszipV2>,
    root_specifier: deno_core::ModuleSpecifier,
    config: IsolateConfig,
    mut request_rx: mpsc::UnboundedReceiver<IsolateRequest>,
    shutdown: CancellationToken,
    metrics: Arc<FunctionMetrics>,
) -> Result<(), Error> {
    // Set up V8 heap limits
    let create_params = if config.max_heap_size_bytes > 0 {
        Some(
            deno_core::v8::CreateParams::default().heap_limits(0, config.max_heap_size_bytes),
        )
    } else {
        None
    };

    // Create JsRuntime with the eszip module loader
    let module_loader = std::rc::Rc::new(EszipModuleLoader::new(eszip));

    let mut runtime_opts = RuntimeOptions {
        module_loader: Some(module_loader),
        create_params,
        extensions: extensions::get_extensions(),
        ..Default::default()
    };
    extensions::set_extension_transpiler(&mut runtime_opts);

    let mut js_runtime = JsRuntime::new(runtime_opts);

    // Register the request handler bridge in the JS global scope.
    // This injects `globalThis.__edgeRuntime` with a request queue.
    handler::inject_request_bridge(&mut js_runtime)?;

    // Load and evaluate the main module (this is where Deno.serve() or
    // the function's handler gets registered).
    let module_id = js_runtime.load_main_es_module(&root_specifier).await?;
    let eval_result = js_runtime.mod_evaluate(module_id);

    js_runtime
        .run_event_loop(PollEventLoopOptions {
            wait_for_inspector: false,
            pump_v8_message_loop: true,
        })
        .await?;

    eval_result.await?;

    info!("function '{}' isolate initialized, entering request loop", name);

    // Request handling loop
    loop {
        tokio::select! {
            Some(req) = request_rx.recv() => {
                metrics.active_requests.fetch_add(1, Ordering::Relaxed);
                metrics.total_requests.fetch_add(1, Ordering::Relaxed);

                let result = handler::dispatch_request(&mut js_runtime, req.request).await;

                metrics.active_requests.fetch_sub(1, Ordering::Relaxed);
                if result.is_err() {
                    metrics.total_errors.fetch_add(1, Ordering::Relaxed);
                }

                // Send response back (ignore if receiver dropped)
                let _ = req.response_tx.send(result);

                // Pump the event loop to process any pending async work
                let _ = js_runtime.run_event_loop(PollEventLoopOptions {
                    wait_for_inspector: false,
                    pump_v8_message_loop: true,
                }).await;
            }
            _ = shutdown.cancelled() => {
                info!("isolate '{}' received shutdown signal", name);
                break;
            }
        }
    }

    Ok(())
}

/// Destroy a function: cancel its isolate and wait for cleanup.
pub async fn destroy_function(entry: &FunctionEntry) {
    if let Some(handle) = &entry.isolate_handle {
        handle.shutdown.cancel();
        // Give the isolate a moment to drain
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
}
