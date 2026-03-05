// Stub for ext:deno_http/00_serve.ts
// In an edge runtime, the HTTP server is provided by the runtime itself,
// not by Node.js http/http2 polyfills. These stubs prevent import errors
// while ensuring the dangerous functionality isn't accidentally used.

const EDGE_RUNTIME_ERROR = "HTTP server APIs are not available in edge runtime. Use the edge runtime's native request handling.";

export function serveHttpOnListener() {
  throw new Error(EDGE_RUNTIME_ERROR);
}

export function upgradeHttpRaw() {
  throw new Error(EDGE_RUNTIME_ERROR);
}

export function serveHttpOnConnection() {
  throw new Error(EDGE_RUNTIME_ERROR);
}

// Additional exports that might be needed
export const HttpConn = class HttpConn {
  constructor() {
    throw new Error(EDGE_RUNTIME_ERROR);
  }
};
