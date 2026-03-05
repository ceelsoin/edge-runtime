// Shim for ext:runtime/98_global_scope_shared.js
// This provides stubs for deno_node modules that expect the full Deno runtime.

// windowOrWorkerGlobalScope provides the global scope objects
// that deno_node's polyfills expect
export const windowOrWorkerGlobalScope = {
  console: {
    value: globalThis.console,
    writable: true,
    enumerable: true,
    configurable: true,
  },
  Window: {
    value: undefined,
  },
  DedicatedWorkerGlobalScope: {
    value: undefined,
  },
};

export default windowOrWorkerGlobalScope;
