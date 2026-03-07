import processMod from "node:process";
import assertMod from "node:assert";
import asyncHooksMod from "node:async_hooks";
import childProcessMod from "node:child_process";
import clusterMod from "node:cluster";
import consoleMod from "node:console";
import diagnosticsChannelMod from "node:diagnostics_channel";
import dgramMod from "node:dgram";
import dnsMod from "node:dns";
import bufferMod from "node:buffer";
import eventsMod from "node:events";
import utilMod from "node:util";
import pathMod from "node:path";
import urlMod from "node:url";
import querystringMod from "node:querystring";
import punycodeMod from "node:punycode";
import streamMod from "node:stream";
import stringDecoderMod from "node:string_decoder";
import osMod from "node:os";
import netMod from "node:net";
import httpMod from "node:http";
import httpsMod from "node:https";
import http2Mod from "node:http2";
import tlsMod from "node:tls";
import perfHooksMod from "node:perf_hooks";
import inspectorMod from "node:inspector";
import readlineMod from "node:readline";
import replMod from "node:repl";
import v8Mod from "node:v8";
import vmMod from "node:vm";
import zlibMod from "node:zlib";
import timersMod from "node:timers";
import timersPromisesMod from "node:timers/promises";
import requestCompatMod from "node:request";
import fsMod from "node:fs";
import fsPromisesMod from "node:fs/promises";

type RequireFn = {
  (id: string): unknown;
  resolve: (id: string) => string;
  cache: Record<string, unknown>;
  main?: unknown;
};

const BUILTIN_RAW = new Map<string, unknown>([
  ["process", processMod],
  ["assert", assertMod],
  ["async_hooks", asyncHooksMod],
  ["child_process", childProcessMod],
  ["cluster", clusterMod],
  ["console", consoleMod],
  ["diagnostics_channel", diagnosticsChannelMod],
  ["dgram", dgramMod],
  ["dns", dnsMod],
  ["buffer", bufferMod],
  ["events", eventsMod],
  ["util", utilMod],
  ["path", pathMod],
  ["url", urlMod],
  ["querystring", querystringMod],
  ["punycode", punycodeMod],
  ["stream", streamMod],
  ["string_decoder", stringDecoderMod],
  ["os", osMod],
  ["net", netMod],
  ["http", httpMod],
  ["https", httpsMod],
  ["http2", http2Mod],
  ["tls", tlsMod],
  ["perf_hooks", perfHooksMod],
  ["inspector", inspectorMod],
  ["readline", readlineMod],
  ["repl", replMod],
  ["v8", v8Mod],
  ["vm", vmMod],
  ["zlib", zlibMod],
  ["timers", timersMod],
  ["timers/promises", timersPromisesMod],
  ["request", requestCompatMod],
  ["fs", fsMod],
  ["fs/promises", fsPromisesMod],
]);

const builtinModules = Object.freeze(
  Array.from(BUILTIN_RAW.keys()).flatMap((name) => [name, `node:${name}`]),
);

const unsupportedRequireMessage = (id: string) =>
  `[edge-runtime] Cannot require '${id}'. Only built-in modules are supported in this runtime profile`;

function normalize(id: string): string {
  if (typeof id !== "string" || id.length === 0) {
    throw new TypeError("require id must be a non-empty string");
  }
  return id.startsWith("node:") ? id.slice(5) : id;
}

function wrapAsCjsLike(exportsValue: unknown): unknown {
  if (!exportsValue || (typeof exportsValue !== "object" && typeof exportsValue !== "function")) {
    return exportsValue;
  }

  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(exportsValue as Record<string, unknown>)) {
    out[k] = v;
  }

  if (!("default" in out)) {
    out.default = exportsValue;
  }
  out.__esModule = true;

  return out;
}

function resolveBuiltin(id: string): string {
  const normalized = normalize(id);
  if (!BUILTIN_RAW.has(normalized)) {
    throw new Error(unsupportedRequireMessage(id));
  }
  return `node:${normalized}`;
}

function createRequire(_filenameOrUrl: string): RequireFn {
  const localCache: Record<string, unknown> = Object.create(null);

  const requireImpl = ((id: string) => {
    const normalized = normalize(id);
    const resolved = resolveBuiltin(id);

    if (Object.prototype.hasOwnProperty.call(localCache, resolved)) {
      return localCache[resolved];
    }

    const mod = BUILTIN_RAW.get(normalized);
    if (mod === undefined) {
      throw new Error(unsupportedRequireMessage(id));
    }

    const cjsExports = wrapAsCjsLike(mod);
    localCache[resolved] = cjsExports;
    return cjsExports;
  }) as RequireFn;

  requireImpl.resolve = (id: string) => resolveBuiltin(id);
  requireImpl.cache = localCache;
  requireImpl.main = undefined;

  return requireImpl;
}

class Module {
  id: string;
  filename: string;
  loaded: boolean;
  exports: Record<string, unknown>;

  constructor(id = "") {
    this.id = id;
    this.filename = id;
    this.loaded = false;
    this.exports = {};
  }

  require(id: string): unknown {
    return createRequire(this.filename)(id);
  }

  static createRequire(filenameOrUrl: string): RequireFn {
    return createRequire(filenameOrUrl);
  }
}

function syncBuiltinESMExports(): void {
  // No-op in this runtime profile.
}

const moduleApi = {
  createRequire,
  Module,
  builtinModules,
  syncBuiltinESMExports,
};

export { createRequire, Module, builtinModules, syncBuiltinESMExports };
export default moduleApi;
