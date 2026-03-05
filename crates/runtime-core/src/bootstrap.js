// Bootstrap module: imports all extension ESM and exposes Web API globals.
//
// deno_core loads extension ESM as side-modules but only evaluates those
// reachable from an esm_entry_point.  This module is that entry point.
// After evaluation we assign the standard Web API classes to globalThis
// so user code can use them (e.g. new Request(), fetch(), etc.).

// -- 1. Import all extension ESM (forces evaluation) -----------

// deno_webidl
import "ext:deno_webidl/00_webidl.js";

// deno_console
import { Console } from "ext:deno_console/01_console.js";

// deno_url
import { URL, URLSearchParams } from "ext:deno_url/00_url.js";
import { URLPattern } from "ext:deno_url/01_urlpattern.js";

// deno_web
import "ext:deno_web/00_infra.js";
import { DOMException } from "ext:deno_web/01_dom_exception.js";
import "ext:deno_web/01_mimesniff.js";
import {
  Event, EventTarget, ErrorEvent, CloseEvent, CustomEvent,
  MessageEvent, ProgressEvent, PromiseRejectionEvent,
  reportError,
} from "ext:deno_web/02_event.js";
import { structuredClone } from "ext:deno_web/02_structured_clone.js";
import {
  setTimeout, setInterval, clearTimeout, clearInterval,
} from "ext:deno_web/02_timers.js";
import { AbortController, AbortSignal } from "ext:deno_web/03_abort_signal.js";
import "ext:deno_web/04_global_interfaces.js";
import { atob, btoa } from "ext:deno_web/05_base64.js";
import {
  ReadableStream, WritableStream, TransformStream,
  ByteLengthQueuingStrategy, CountQueuingStrategy,
} from "ext:deno_web/06_streams.js";
import {
  TextEncoder, TextDecoder, TextEncoderStream, TextDecoderStream,
} from "ext:deno_web/08_text_encoding.js";
import { Blob, File } from "ext:deno_web/09_file.js";
import { FileReader } from "ext:deno_web/10_filereader.js";
import "ext:deno_web/12_location.js";
import { MessageChannel, MessagePort } from "ext:deno_web/13_message_port.js";
import { CompressionStream, DecompressionStream } from "ext:deno_web/14_compression.js";
import { Performance, performance, PerformanceEntry, PerformanceMark, PerformanceMeasure } from "ext:deno_web/15_performance.js";
import { ImageData } from "ext:deno_web/16_image_data.js";

// deno_crypto
import { Crypto, crypto, CryptoKey, SubtleCrypto } from "ext:deno_crypto/00_crypto.js";

// deno_telemetry
import "ext:deno_telemetry/telemetry.ts";
import "ext:deno_telemetry/util.ts";

// deno_fetch
import { Headers } from "ext:deno_fetch/20_headers.js";
import { FormData } from "ext:deno_fetch/21_formdata.js";
import "ext:deno_fetch/22_body.js";
import "ext:deno_fetch/22_http_client.js";
import { Request } from "ext:deno_fetch/23_request.js";
import { Response } from "ext:deno_fetch/23_response.js";
import { fetch } from "ext:deno_fetch/26_fetch.js";
import { EventSource } from "ext:deno_fetch/27_eventsource.js";

// deno_net
import "ext:deno_net/01_net.js";
import "ext:deno_net/02_tls.js";

// -- 2. Expose Web API globals on globalThis ---------------------

// console
const core = globalThis.Deno?.core ?? globalThis.__bootstrap?.core;
if (!globalThis.console) {
  globalThis.console = new Console((msg, level) => {
    core?.print?.(msg, level > 1);
  });
}

// Deno namespace (minimal, for Deno.serve interception)
if (!globalThis.Deno) {
  globalThis.Deno = {};
}

// URL
Object.assign(globalThis, {
  URL,
  URLSearchParams,
  URLPattern,
});

// Events
Object.assign(globalThis, {
  Event,
  EventTarget,
  ErrorEvent,
  CloseEvent,
  CustomEvent,
  MessageEvent,
  ProgressEvent,
  PromiseRejectionEvent,
  reportError,
});

// Timers
Object.assign(globalThis, {
  setTimeout,
  setInterval,
  clearTimeout,
  clearInterval,
});

// Abort
Object.assign(globalThis, {
  AbortController,
  AbortSignal,
});

// Encoding
Object.assign(globalThis, {
  atob,
  btoa,
  TextEncoder,
  TextDecoder,
  TextEncoderStream,
  TextDecoderStream,
});

// Streams
Object.assign(globalThis, {
  ReadableStream,
  WritableStream,
  TransformStream,
  ByteLengthQueuingStrategy,
  CountQueuingStrategy,
});

// DOM
Object.assign(globalThis, {
  DOMException,
  structuredClone,
});

// Files
Object.assign(globalThis, {
  Blob,
  File,
  FileReader,
});

// Compression
Object.assign(globalThis, {
  CompressionStream,
  DecompressionStream,
});

// Performance
Object.assign(globalThis, {
  Performance,
  performance,
  PerformanceEntry,
  PerformanceMark,
  PerformanceMeasure,
});

// Messaging
Object.assign(globalThis, {
  MessageChannel,
  MessagePort,
  ImageData,
});

// Crypto
Object.assign(globalThis, {
  Crypto,
  crypto,
  CryptoKey,
  SubtleCrypto,
});

// Fetch
Object.assign(globalThis, {
  Headers,
  FormData,
  Request,
  Response,
  fetch,
  EventSource,
});
