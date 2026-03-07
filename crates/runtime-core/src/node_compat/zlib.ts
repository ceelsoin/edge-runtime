type NodeLikeError = Error & { code?: string };

function notImplemented(api: string): never {
  const err = new Error(
    `[edge-runtime] ${api} is not implemented in this runtime profile`,
  ) as NodeLikeError;
  err.code = "ERR_NOT_IMPLEMENTED";
  throw err;
}

function runtimeCompressionStreams(): {
  CompressionStreamCtor: typeof CompressionStream;
  DecompressionStreamCtor: typeof DecompressionStream;
} {
  if (typeof globalThis.CompressionStream !== "function") {
    notImplemented("zlib (CompressionStream is not available)");
  }
  if (typeof globalThis.DecompressionStream !== "function") {
    notImplemented("zlib (DecompressionStream is not available)");
  }
  return {
    CompressionStreamCtor: globalThis.CompressionStream,
    DecompressionStreamCtor: globalThis.DecompressionStream,
  };
}

function toBytes(input: unknown): Uint8Array {
  if (input instanceof Uint8Array) return input;
  if (input instanceof ArrayBuffer) return new Uint8Array(input);
  if (ArrayBuffer.isView(input)) {
    return new Uint8Array(input.buffer, input.byteOffset, input.byteLength);
  }
  if (typeof input === "string") {
    return new TextEncoder().encode(input);
  }
  return new TextEncoder().encode(String(input ?? ""));
}

function toNodeBufferLike(bytes: Uint8Array): unknown {
  const BufferCtor = (globalThis as unknown as { Buffer?: { from: (arg: Uint8Array) => unknown } }).Buffer;
  if (BufferCtor?.from) return BufferCtor.from(bytes);
  return bytes;
}

async function transformBytes(
  input: unknown,
  format: "gzip" | "deflate" | "deflate-raw",
  mode: "compress" | "decompress",
): Promise<unknown> {
  const { CompressionStreamCtor, DecompressionStreamCtor } = runtimeCompressionStreams();
  const data = toBytes(input);

  const source = new Blob([data]).stream();
  const transform = mode === "compress"
    ? new CompressionStreamCtor(format)
    : new DecompressionStreamCtor(format);

  const output = source.pipeThrough(transform);
  const outBuf = await new Response(output).arrayBuffer();
  return toNodeBufferLike(new Uint8Array(outBuf));
}

type ZlibCallback = (err: unknown, result?: unknown) => void;

function resolveCallback(
  optionsOrCb?: unknown,
  maybeCb?: unknown,
): ZlibCallback | undefined {
  if (typeof optionsOrCb === "function") return optionsOrCb as ZlibCallback;
  if (typeof maybeCb === "function") return maybeCb as ZlibCallback;
  return undefined;
}

function runWithCallback(
  op: Promise<unknown>,
  cb?: ZlibCallback,
): Promise<unknown> | void {
  if (typeof cb !== "function") return op;
  void op.then((result) => cb(null, result)).catch((err) => cb(err));
  return undefined;
}

function createGzip(): never {
  return notImplemented("zlib.createGzip");
}

function createGunzip(): never {
  return notImplemented("zlib.createGunzip");
}

function gzip(
  input: unknown,
  optionsOrCb?: unknown,
  maybeCb?: unknown,
): Promise<unknown> | void {
  const cb = resolveCallback(optionsOrCb, maybeCb);
  return runWithCallback(transformBytes(input, "gzip", "compress"), cb);
}

function gunzip(
  input: unknown,
  optionsOrCb?: unknown,
  maybeCb?: unknown,
): Promise<unknown> | void {
  const cb = resolveCallback(optionsOrCb, maybeCb);
  return runWithCallback(transformBytes(input, "gzip", "decompress"), cb);
}

function deflate(
  input: unknown,
  optionsOrCb?: unknown,
  maybeCb?: unknown,
): Promise<unknown> | void {
  const cb = resolveCallback(optionsOrCb, maybeCb);
  return runWithCallback(transformBytes(input, "deflate", "compress"), cb);
}

function inflate(
  input: unknown,
  optionsOrCb?: unknown,
  maybeCb?: unknown,
): Promise<unknown> | void {
  const cb = resolveCallback(optionsOrCb, maybeCb);
  return runWithCallback(transformBytes(input, "deflate", "decompress"), cb);
}

function deflateRaw(
  input: unknown,
  optionsOrCb?: unknown,
  maybeCb?: unknown,
): Promise<unknown> | void {
  const cb = resolveCallback(optionsOrCb, maybeCb);
  return runWithCallback(transformBytes(input, "deflate-raw", "compress"), cb);
}

function inflateRaw(
  input: unknown,
  optionsOrCb?: unknown,
  maybeCb?: unknown,
): Promise<unknown> | void {
  const cb = resolveCallback(optionsOrCb, maybeCb);
  return runWithCallback(transformBytes(input, "deflate-raw", "decompress"), cb);
}

function brotliCompress(): never {
  return notImplemented("zlib.brotliCompress");
}

function brotliDecompress(): never {
  return notImplemented("zlib.brotliDecompress");
}

function gzipSync(): never {
  return notImplemented("zlib.gzipSync");
}

function gunzipSync(): never {
  return notImplemented("zlib.gunzipSync");
}

function deflateSync(): never {
  return notImplemented("zlib.deflateSync");
}

function inflateSync(): never {
  return notImplemented("zlib.inflateSync");
}

function deflateRawSync(): never {
  return notImplemented("zlib.deflateRawSync");
}

function inflateRawSync(): never {
  return notImplemented("zlib.inflateRawSync");
}

function brotliCompressSync(): never {
  return notImplemented("zlib.brotliCompressSync");
}

function brotliDecompressSync(): never {
  return notImplemented("zlib.brotliDecompressSync");
}

const constants = {
  Z_NO_FLUSH: 0,
  Z_FINISH: 4,
  Z_OK: 0,
  Z_STREAM_END: 1,
};

const zlibModule = {
  createGzip,
  createGunzip,
  gzip,
  gunzip,
  deflate,
  inflate,
  deflateRaw,
  inflateRaw,
  brotliCompress,
  brotliDecompress,
  gzipSync,
  gunzipSync,
  deflateSync,
  inflateSync,
  deflateRawSync,
  inflateRawSync,
  brotliCompressSync,
  brotliDecompressSync,
  constants,
};

export {
  createGzip,
  createGunzip,
  gzip,
  gunzip,
  deflate,
  inflate,
  deflateRaw,
  inflateRaw,
  brotliCompress,
  brotliDecompress,
  gzipSync,
  gunzipSync,
  deflateSync,
  inflateSync,
  deflateRawSync,
  inflateRawSync,
  brotliCompressSync,
  brotliDecompressSync,
  constants,
};
export default zlibModule;
