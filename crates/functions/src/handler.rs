use anyhow::Error;
use deno_core::JsRuntime;

/// Inject the request/response bridge into the JS global scope.
///
/// This creates a `globalThis.__edgeRuntime` object with:
/// - `__edgeRuntime.handler`: the registered fetch handler (set by user code)
/// - `__edgeRuntime.registerHandler(fn)`: called by the user's `Deno.serve()` equivalent
///
/// The user's JS code should call:
/// ```js
/// globalThis.__edgeRuntime.registerHandler(async (req) => {
///   return new Response("Hello!");
/// });
/// ```
///
/// Or we can override `Deno.serve` to do this automatically.
pub fn inject_request_bridge(js_runtime: &mut JsRuntime) -> Result<(), Error> {
    js_runtime.execute_script(
        "edge-internal:///runtime_bridge.js",
        deno_core::ascii_str!(
            r#"
            globalThis.__edgeRuntime = {
                handler: null,
                registerHandler(fn) {
                    this.handler = fn;
                },
            };

            // Override Deno.serve to capture the handler
            const originalServe = globalThis.Deno?.serve;
            if (globalThis.Deno) {
                globalThis.Deno.serve = function(handlerOrOptions, maybeHandler) {
                    let handler;
                    if (typeof handlerOrOptions === 'function') {
                        handler = handlerOrOptions;
                    } else if (typeof maybeHandler === 'function') {
                        handler = maybeHandler;
                    } else if (handlerOrOptions && typeof handlerOrOptions.handler === 'function') {
                        handler = handlerOrOptions.handler;
                    } else if (handlerOrOptions && typeof handlerOrOptions.fetch === 'function') {
                        handler = handlerOrOptions.fetch;
                    }
                    if (handler) {
                        globalThis.__edgeRuntime.registerHandler(handler);
                    }
                    // Return a mock server object
                    return {
                        finished: new Promise(() => {}),
                        ref() {},
                        unref() {},
                        shutdown() { return Promise.resolve(); },
                        addr: { hostname: "0.0.0.0", port: 0, transport: "tcp" },
                    };
                };
            }

            // Also support addEventListener('fetch', ...) style
            globalThis.__edgeRuntime._fetchListeners = [];
            globalThis.addEventListener = function(type, listener) {
                if (type === 'fetch') {
                    globalThis.__edgeRuntime._fetchListeners.push(listener);
                    // Wrap as handler
                    globalThis.__edgeRuntime.registerHandler(async (req) => {
                        let response = null;
                        const event = {
                            request: req,
                            respondWith(r) { response = r; },
                        };
                        listener(event);
                        return await response;
                    });
                }
            };

            // Expose a function for Rust to call
            globalThis.__edgeRuntime.handleRequest = async function(method, url, headersJson, body) {
                const handler = globalThis.__edgeRuntime.handler;
                if (!handler) {
                    return JSON.stringify({
                        status: 503,
                        headers: { "content-type": "application/json" },
                        body: '{"error":"no handler registered"}',
                    });
                }

                try {
                    const headers = JSON.parse(headersJson || '{}');
                    const reqInit = {
                        method: method,
                        headers: new Headers(headers),
                    };
                    if (body && body.length > 0 && method !== 'GET' && method !== 'HEAD') {
                        reqInit.body = body;
                    }
                    const request = new Request(url, reqInit);
                    const response = await handler(request);

                    const respHeaders = {};
                    response.headers.forEach((value, key) => {
                        respHeaders[key] = value;
                    });

                    const respBody = await response.text();

                    return JSON.stringify({
                        status: response.status,
                        headers: respHeaders,
                        body: respBody,
                    });
                } catch (err) {
                    return JSON.stringify({
                        status: 500,
                        headers: { "content-type": "application/json" },
                        body: JSON.stringify({ error: String(err) }),
                    });
                }
            };
            "#
        ),
    )?;
    Ok(())
}

/// JSON shape returned by __edgeRuntime.handleRequest
#[derive(serde::Deserialize)]
struct JsResponse {
    status: u16,
    headers: std::collections::HashMap<String, String>,
    body: String,
}

/// Dispatch an HTTP request into the JS fetch handler and return the response.
pub async fn dispatch_request(
    js_runtime: &mut JsRuntime,
    request: http::Request<bytes::Bytes>,
) -> Result<http::Response<bytes::Bytes>, Error> {
    let method = request.method().to_string();

    // Build an absolute URL — `new Request(url)` in JS requires one.
    // The router forwards only the rewritten path (e.g. "/"), so we
    // reconstruct the full URL from the Host header.
    let host = request
        .headers()
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost");
    let path = request.uri().path_and_query().map_or("/", |pq| pq.as_str());
    let uri = format!("http://{host}{path}");

    // Serialize headers to JSON
    let headers_map: std::collections::HashMap<String, String> = request
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();
    let headers_json = serde_json::to_string(&headers_map)?;

    let body = request.into_body();

    // Call globalThis.__edgeRuntime.handleRequest(...) directly via V8 API,
    // avoiding dynamic execute_script frames on every request.
    let result_global = {
        let context = js_runtime.main_context();
        let isolate = js_runtime.v8_isolate();
        let mut handle_scope = deno_core::v8::HandleScope::new(isolate);
        let mut handle_scope = {
            let pinned = unsafe { std::pin::Pin::new_unchecked(&mut handle_scope) };
            pinned.init()
        };
        let scope = &mut handle_scope;
        let context = deno_core::v8::Local::new(scope, context);
        let scope = &mut deno_core::v8::ContextScope::new(scope, context);

        let global = context.global(scope);
        let edge_runtime_key = deno_core::v8::String::new(scope, "__edgeRuntime")
            .ok_or_else(|| anyhow::anyhow!("failed to allocate __edgeRuntime key"))?;
        let edge_runtime_val = global
            .get(scope, edge_runtime_key.into())
            .ok_or_else(|| anyhow::anyhow!("globalThis.__edgeRuntime is missing"))?;
        let edge_runtime_obj = edge_runtime_val
            .to_object(scope)
            .ok_or_else(|| anyhow::anyhow!("globalThis.__edgeRuntime is not an object"))?;

        let handle_request_key = deno_core::v8::String::new(scope, "handleRequest")
            .ok_or_else(|| anyhow::anyhow!("failed to allocate handleRequest key"))?;
        let handle_request_val = edge_runtime_obj
            .get(scope, handle_request_key.into())
            .ok_or_else(|| anyhow::anyhow!("__edgeRuntime.handleRequest is missing"))?;
        let handle_request_fn = deno_core::v8::Local::<deno_core::v8::Function>::try_from(handle_request_val)
            .map_err(|_| anyhow::anyhow!("__edgeRuntime.handleRequest is not a function"))?;

        let method_v8 = deno_core::v8::String::new(scope, &method)
            .ok_or_else(|| anyhow::anyhow!("failed to allocate method string"))?;
        let uri_v8 = deno_core::v8::String::new(scope, &uri)
            .ok_or_else(|| anyhow::anyhow!("failed to allocate uri string"))?;
        let headers_v8 = deno_core::v8::String::new(scope, &headers_json)
            .ok_or_else(|| anyhow::anyhow!("failed to allocate headers string"))?;

        let body_arg: deno_core::v8::Local<deno_core::v8::Value> = if body.is_empty() {
            deno_core::v8::null(scope).into()
        } else {
            let backing_store = deno_core::v8::ArrayBuffer::new_backing_store_from_boxed_slice(
                body.to_vec().into_boxed_slice(),
            );
            let backing_store = backing_store.make_shared();
            let array_buffer = deno_core::v8::ArrayBuffer::with_backing_store(scope, &backing_store);
            let uint8 = deno_core::v8::Uint8Array::new(scope, array_buffer, 0, body.len())
                .ok_or_else(|| anyhow::anyhow!("failed to allocate Uint8Array body"))?;
            uint8.into()
        };

        let args: [deno_core::v8::Local<deno_core::v8::Value>; 4] = [
            method_v8.into(),
            uri_v8.into(),
            headers_v8.into(),
            body_arg,
        ];

        let result = handle_request_fn
            .call(scope, edge_runtime_obj.into(), &args)
            .ok_or_else(|| anyhow::anyhow!("failed to call __edgeRuntime.handleRequest"))?;

        deno_core::v8::Global::new(scope, result)
    };

    // The result is a Promise, we need to resolve it.
    let resolved = js_runtime.resolve(result_global);

    // Run the event loop to resolve the promise
    js_runtime
        .run_event_loop(deno_core::PollEventLoopOptions {
            wait_for_inspector: false,
            pump_v8_message_loop: true,
        })
        .await?;

    let resolved_value = resolved.await?;

    // Extract the JSON string from the resolved value
    // Create a HandleScope and ContextScope for V8 operations
    let context = js_runtime.main_context();
    let isolate = js_runtime.v8_isolate();
    let mut handle_scope = deno_core::v8::HandleScope::new(isolate);
    let mut handle_scope = {
        let pinned = unsafe { std::pin::Pin::new_unchecked(&mut handle_scope) };
        pinned.init()
    };
    let scope = &mut handle_scope;
    let context = deno_core::v8::Local::new(scope, context);
    let scope = &mut deno_core::v8::ContextScope::new(scope, context);

    let local = deno_core::v8::Local::new(scope, resolved_value);
    let json_str = local
        .to_string(scope)
        .ok_or_else(|| anyhow::anyhow!("failed to convert JS result to string"))?
        .to_rust_string_lossy(scope);

    // Parse the JSON response
    let js_response: JsResponse = serde_json::from_str(&json_str)
        .map_err(|e| anyhow::anyhow!("failed to parse JS response: {e}, got: {json_str}"))?;

    // Build the HTTP response
    let mut builder = http::Response::builder().status(js_response.status);

    for (key, value) in &js_response.headers {
        builder = builder.header(key.as_str(), value.as_str());
    }

    let response = builder
        .body(bytes::Bytes::from(js_response.body))
        .map_err(|e| anyhow::anyhow!("failed to build HTTP response: {e}"))?;

    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use deno_core::RuntimeOptions;
    use runtime_core::extensions;

    static INIT: std::sync::Once = std::sync::Once::new();

    fn init_v8() {
        INIT.call_once(|| {
            deno_core::JsRuntime::init_platform(None);
        });
    }

    fn make_runtime() -> JsRuntime {
        init_v8();
        let mut opts = RuntimeOptions {
            extensions: extensions::get_extensions(),
            ..Default::default()
        };
        extensions::set_extension_transpiler(&mut opts);
        JsRuntime::new(opts)
    }

    #[test]
    fn inject_bridge_sets_globals() {
        let mut runtime = make_runtime();
        inject_request_bridge(&mut runtime).expect("inject_request_bridge failed");

        let val = runtime
            .execute_script(
                "<test>",
                deno_core::ascii_str!("typeof globalThis.__edgeRuntime === 'object'"),
            )
            .unwrap();

        deno_core::scope!(scope, runtime);
        let local = val.open(scope);
        assert!(local.is_true(), "__edgeRuntime should be an object on globalThis");
    }

    #[test]
    fn inject_bridge_overrides_deno_serve() {
        let mut runtime = make_runtime();
        inject_request_bridge(&mut runtime).expect("inject_request_bridge failed");

        let val = runtime
            .execute_script(
                "<test>",
                deno_core::ascii_str!("typeof globalThis.Deno.serve === 'function'"),
            )
            .unwrap();

        deno_core::scope!(scope, runtime);
        let local = val.open(scope);
        assert!(local.is_true(), "Deno.serve should be a function");
    }

    #[test]
    fn dispatch_without_handler_returns_503() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let local = tokio::task::LocalSet::new();
        let result = local.block_on(&rt, async {
            let mut runtime = make_runtime();
            inject_request_bridge(&mut runtime).expect("inject_request_bridge failed");

            let request = http::Request::builder()
                .method("GET")
                .uri("/test")
                .header("host", "localhost:9000")
                .body(bytes::Bytes::new())
                .unwrap();

            dispatch_request(&mut runtime, request).await
        });

        let response = result.expect("dispatch_request should not error");
        assert_eq!(response.status(), 503, "should return 503 when no handler registered");
    }
}
