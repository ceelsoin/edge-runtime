# Security & Architecture Audit — Deno Edge Runtime

> **Date:** 03/05/2026
> **Scope:** Complete analysis of the 4 crates (`runtime-core`, `functions`, `server`, `cli`), JS tests, scripts, and configuration.
> **Objective:** Identify security vulnerabilities, breaking points, design flaws, and test gaps before production use.

---

## Summary

| Severity | Count | Examples |
|---|---|---|
| **Critical** | 4 | TLS not applied, unauthenticated endpoints, SSRF, unbounded body |
| **High** | 6 | No connection limit, imprecise CPU timer, panic without recovery, heap without enforcement |
| **Medium** | 8 | Inactive rate limiter, expensive metrics, exposed inspector, hardcoded paths |
| **Low** | 5 | Ordering::Relaxed, error messages leaking info, mutable globals |

---

## 1. Critical Security Vulnerabilities

### 1.1 TLS Configured but Never Used

**File:** `crates/server/src/lib.rs` (lines 46-48)
**Severity:** 🔴 CRITICAL

The TLS acceptor is built but stored in `_tls_acceptor` (`_` prefix = unused). All connections are served in **plain HTTP**, even when certificate and key are provided. The operator believes TLS is active, but traffic is cleartext.

```rust
let _tls_acceptor = if let Some(ref tls_config) = config.tls {
    Some(tls::build_tls_acceptor(tls_config)?)  // Built and discarded!
} else { None };
```

**Impact:** Sensitive data exposed on the network. False sense of security.

**Fix:** Use the `tls_acceptor` to wrap the accepted TCP stream with `tokio_rustls::TlsAcceptor::accept()`.

---

### 1.2 Management Endpoints WITHOUT Authentication

**File:** `crates/server/src/router.rs`
**Severity:** 🔴 CRITICAL

The `/_internal/*` endpoints (deploy, delete, metrics) are completely open. Any client on the network can:

- **Deploy malicious functions** via `POST /_internal/functions`
- **Delete production functions** via `DELETE /_internal/functions/{name}`
- **Extract internal information** via `GET /_internal/metrics`

No auth header, API key, or mTLS is verified. Searching for `auth|token|api.key|bearer|secret` in the router returns zero results.

**Impact:** Complete runtime takeover by any attacker with network access.

**Fix:** Implement authentication via API key (header `X-API-Key`), mTLS, or JWT on `/_internal/*` endpoints.

---

### 1.3 SSRF via `fetch()` without Private IP Restriction

**File:** `crates/runtime-core/src/permissions.rs` (line 23)
**Severity:** 🔴 CRITICAL

The default network permission uses `allow_net: Some(vec![])` — which in Deno means **allow ALL hosts**. User code can do:

```javascript
// Access cloud metadata (AWS, GCP, Azure)
fetch("http://169.254.169.254/latest/meta-data/iam/security-credentials/")

// Scan internal network
fetch("http://192.168.1.1:8080/admin")

// Access localhost services
fetch("http://127.0.0.1:5432/")
```

There is no validation of private IPs (RFC 1918, link-local, loopback).

**Impact:** Cloud credential exfiltration, internal network scanning, access to unexposed services.

**Fix:** Implement deny list for private ranges: `10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`, `169.254.0.0/16`, `127.0.0.0/8`, `::1`, `fc00::/7`.

---

### 1.4 No Size Limit on Request/Response Body

**File:** `crates/server/src/router.rs` (lines 73-80, 214-220)
**Severity:** 🔴 CRITICAL

All HTTP body is fully buffered in memory via `body.collect().await`. There is no maximum `Content-Length` check nor streaming. A 10GB payload would cause instant OOM.

```rust
let body_bytes = match body.collect().await {
    Ok(collected) => collected.to_bytes(),
    Err(_) => { return json_response(...) }
};
```

The same occurs on the response side — the handler serializes the entire body into `bytes::Bytes` without limit.

**Impact:** Denial of Service via large payload. Crash of entire process.

**Fix:** Limit body to 5-10MB with reject `413 Payload Too Large`. Implement streaming for larger payloads.

---

## 2. High Severity Vulnerabilities

### 2.1 No Limit on Simultaneous Connections

**File:** `crates/server/src/lib.rs` (lines 53-68)
**Severity:** 🟠 HIGH

The accept loop does `tokio::spawn` for each connection without semaphore, queue, or backpressure. A slowloris attack or connection flood exhausts threads and memory of the Tokio runtime.

```rust
Ok((stream, peer_addr)) => {
    let svc = svc.clone();
    tokio::spawn(async move { ... });  // No limit!
}
```

**Fix:** Use `tokio::sync::Semaphore` with configurable limit (e.g., 10,000 connections).

---

### 2.2 CPU Timer Uses Wall-Clock (Not Real CPU Time)

**File:** `crates/runtime-core/src/cpu_timer.rs` (lines 5-6)
**Severity:** 🟠 HIGH

The code itself comments: *"Uses wall-clock time as an approximation."* This means:

- Code that does `await sleep(50_000)` consumes 0% CPU but exceeds the timer
- CPU-intensive code in async microtasks may not be detected
- `Ordering::Relaxed` on atomics may lose updates between threads

**Fix:** Use `clock_gettime(CLOCK_THREAD_CPUTIME_ID)` via `libc` (already a dependency). Or at minimum, prominently document the limitation.

---

### 2.3 Heap Limit without Real Enforcement

**File:** `crates/runtime-core/src/mem_check.rs` (lines 10-15) and `crates/functions/src/lifecycle.rs`
**Severity:** 🟠 HIGH

V8's `CreateParams::heap_limits()` is set, but:
- No `near_heap_limit_callback` is registered
- `mem_check::check()` returns `(used, exceeded)` but **does not kill** the isolate
- `used_heap_size + external_memory` does not include code cache, compiled functions, or native buffers
- The caller must check the flag manually — nothing prevents ignoring it

**Impact:** Isolate can exceed the limit and cause process OOM.

**Fix:** Register `v8::Isolate::add_near_heap_limit_callback()` to terminate the isolate before OOM.

---

### 2.4 Panic in Isolate Does Not Update Status

**File:** `crates/functions/src/lifecycle.rs` (lines 63-76)
**Severity:** 🟠 HIGH

`catch_unwind` captures panics but the function status remains `Running` in the registry:

```rust
Err(e) => error!("isolate '{}' panicked: {:?}", isolate_name, e),
// But never calls registry.update_status(name, Status::Error)
```

Subsequent requests are sent to a dead isolate and hang without response.

**Fix:** After panic, update status to `Error` in registry and implement auto-restart with backoff.

---

### 2.5 No Request Timeout in Dispatch

**File:** `crates/functions/src/handler.rs` (lines 126-185)
**Severity:** 🟠 HIGH

`dispatch_request` awaits the JS handler indefinitely. Malicious code or infinite loops block the isolate permanently:

```rust
let result = handler::dispatch_request(&mut js_runtime, req.request).await;
// No tokio::time::timeout!
```

**Note:** router.rs applies timeout on `send_request`, but the internal isolate has no timeout of its own, potentially accumulating pending requests.

**Fix:** Wrap with `tokio::time::timeout()` and return 504 Gateway Timeout.

---

### 2.6 CPU Timer `exceeded` Flag Is Never Reset

**File:** `crates/runtime-core/src/cpu_timer.rs` (line 32)
**Severity:** 🟠 HIGH

Once `exceeded.store(true)`, the flag remains `true` forever. Multiple requests to the same isolate would inherit the timeout status from a previous request.

**Fix:** Add a `reset()` method and call it at the start of each request.

---

## 3. Medium Severity Vulnerabilities

### 3.1 Rate Limiting Defined but Never Applied

**File:** `crates/server/src/middleware/mod.rs`
**Severity:** 🟡 MEDIUM

The `RateLimitLayer` module is implemented but never inserted into the server's middleware stack in `lib.rs`. The `rate_limit_rps` config is accepted but ignored.

---

### 3.2 Metrics Endpoint Is Computationally Expensive (No Cache)

**File:** `crates/server/src/router.rs` (lines ~130-175)
**Severity:** 🟡 MEDIUM

Each call to `GET /_internal/metrics` executes `sysinfo::System::new_all()` + `refresh_processes()`, which scans **all OS processes**. Can be used as a DoS vector (1000 req/s to metrics = 100% CPU).

**Fix:** Cache the result with 10-30 second TTL.

---

### 3.3 V8 Inspector Without Network Protection

**File:** `crates/runtime-core/src/isolate.rs` (lines 24-27)
**Severity:** 🟡 MEDIUM

The V8 Inspector (debugger) can be enabled via CLI flags `--inspect` / `--inspect-brk`. There is no documentation on whether binding is restricted to localhost. In production, this would give remote access to each isolate's V8 debugger.

**Fix:** Force bind to `127.0.0.1` and document that inspector should not be used in production.

---

### 3.4 Hardcoded Absolute Paths in CLI

**Files:** `crates/cli/src/bundle.rs`, `crates/cli/src/watch.rs`, `crates/cli/src/test.rs`
**Severity:** 🟡 MEDIUM

Paths like `cwd.join("crates/runtime-core/src/assert/...")` assume CWD is the project root. Fails silently in any other directory.

**Fix:** Use environment variable or auto-detect project root via `Cargo.toml`.

---

### 3.5 Function Name Without Validation

**File:** `crates/server/src/router.rs` (lines 52-62)
**Severity:** 🟡 MEDIUM

The function name is extracted from the URL path without any validation:

```rust
let segments: Vec<&str> = path.splitn(3, '/').collect();
let function_name = if segments.len() >= 2 { segments[1] } else { "" };
```

Accepts special characters, unicode, very long strings, `..`, `/`, etc. May allow path traversal or enumeration.

**Fix:** Validate regex `^[a-z0-9][a-z0-9-]{0,62}$`.

---

### 3.6 Silent Stub Ops Instead of Errors

**File:** `crates/runtime-core/src/extensions.rs` (lines 98-108)
**Severity:** 🟡 MEDIUM

`op_set_raw`, `op_console_size`, `op_tls_peer_certificate` return no-op/default silently. If user code invokes these ops, silent failure may mask problems.

---

### 3.7 Source Maps Enabled by Default

**File:** `crates/runtime-core/src/isolate.rs` (line 28)
**Severity:** 🟡 MEDIUM

`enable_source_maps: true` by default embeds TypeScript source in base64 in compiled JS. May expose business logic, internal comments, and file paths.

---

### 3.8 Error Messages Leak Internal Information

**File:** `crates/server/src/router.rs` (line ~246)
**Severity:** 🟡 MEDIUM

```rust
format!(r#"{{"error":"{}"}}"#, e)  // 'e' contains Rust stack traces
```

Deploy error messages include isolate stack details, server file paths, and internal state.

**Fix:** Return generic message to client; log details internally.

---

## 4. Low Severity Vulnerabilities

### 4.1 Ordering::Relaxed on Metrics Atomics

**File:** `crates/functions/src/types.rs` (lines 75-93)

Metrics snapshot may show inconsistent values between fields. Use `Ordering::Acquire` at minimum.

### 4.2 Mutable Globals in Bootstrap

**File:** `crates/runtime-core/src/bootstrap.js` (lines 95-213)

50+ APIs assigned to `globalThis` without `Object.freeze()`. User code can overwrite `fetch`, `crypto`, `Response`, etc.

### 4.3 Silent Exception in edge_assert Import

**File:** `crates/runtime-core/src/bootstrap.js` (lines 76-80)

```javascript
import("ext:edge_assert/mod.ts").catch(() => {});  // Silences any error
```

### 4.4 Custom HTTP Parser in Test Runner Inspector

**File:** `crates/cli/src/test.rs` (lines 656-787)

Manual HTTP parser is not RFC-compliant, no request size limit, no origin validation in WebSocket.

### 4.5 Graceful Shutdown with Fixed Sleep in Registry

**File:** `crates/functions/src/registry.rs` (lines ~133-137)

`sleep(2s)` + `functions.clear()` without verifying if threads have terminated.

---

## 5. Test Coverage Gaps

| Category | Status |
|---|---|
| Permission enforcement (network denied, fs denied) | ❌ Missing |
| Memory limit (OOM) | ❌ Missing |
| CPU timeout (infinite loop) | ❌ Missing |
| Isolate panic recovery | ❌ Missing |
| Concurrent requests to same isolate | ❌ Missing |
| Graceful shutdown with in-flight requests | ❌ Missing |
| SSRF (fetch to private IPs) | ❌ Missing |
| Maximum body size | ❌ Missing |
| Prototype pollution / sandbox escape | ❌ Missing |
| Negative tests for Web APIs (invalid inputs) | ❌ Nearly zero |
| Internal endpoint authentication | ❌ Missing |
| End-to-end TLS handshake | ❌ Missing |
| Web APIs existence/constructors | ✅ OK (~70 APIs) |
| Isolate boot and basic dispatch | ✅ OK |
| Load testing (k6) | ✅ OK |

---

## 6. Positive Observations

The fundamental architecture is solid:

- **Sound technology choice:** Deno core + V8 + eszip + hyper + tower is a robust and modern stack
- **Well-defined crate separation:** `runtime-core` (sandbox), `functions` (lifecycle), `server` (HTTP), `cli` (tooling)
- **Deno permissions used correctly** (deny fs, deny env, deny ffi, deny run)
- **CancellationToken for shutdown** is the correct pattern
- **DashMap for registry** is a good choice for concurrency
- **Observability with OpenTelemetry** already in dependencies
- **Broad Web API coverage** in compatibility tests
- **Well-organized examples** with 25+ use cases

The issues are primarily about **enforcement** (mechanisms configured but not activated) and **hardening** (input validation, resource limits, authentication) — not fundamental design.
