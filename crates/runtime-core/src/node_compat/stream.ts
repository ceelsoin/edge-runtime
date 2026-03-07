import { EventEmitter } from "node:events";

type Callback = (err?: unknown, value?: unknown) => void;

type AbortSignalLike = {
  aborted: boolean;
  reason?: unknown;
  addEventListener: (type: string, listener: () => void, options?: unknown) => void;
  removeEventListener: (type: string, listener: () => void) => void;
};

type PipelineOptions = {
  signal?: AbortSignalLike;
};

class Stream extends EventEmitter {}

class Readable extends Stream {
  readable = true;
  destroyed = false;
  readableEnded = false;
  #paused = false;
  #buffer: unknown[] = [];
  #highWaterMark: number;

  constructor(options: Record<string, unknown> = {}) {
    super();
    this.#highWaterMark = (options.highWaterMark as number) || 16384;
  }

  static from(iterable: Iterable<unknown> | AsyncIterable<unknown>) {
    const readable = new Readable();

    const pump = async () => {
      try {
        for await (const chunk of iterable as AsyncIterable<unknown>) {
          readable.push(chunk);
        }
        readable.push(null);
      } catch (err) {
        readable.emit("error", err);
      }
    };

    queueMicrotask(() => {
      void pump();
    });

    return readable;
  }

  static fromWeb(webReadable: ReadableStream<unknown>) {
    if (!webReadable || typeof (webReadable as ReadableStream<unknown>).getReader !== "function") {
      throw new TypeError("Readable.fromWeb expects a ReadableStream");
    }

    const readable = new Readable();
    const reader = webReadable.getReader();

    const pump = async () => {
      try {
        while (!readable.destroyed) {
          const { done, value } = await reader.read();
          if (done) {
            readable.push(null);
            break;
          }
          readable.push(value);
        }
      } catch (err) {
        readable.emit("error", err);
      } finally {
        try {
          reader.releaseLock();
        } catch {
          // Ignore lock release errors.
        }
      }
    };

    queueMicrotask(() => {
      void pump();
    });

    const originalDestroy = readable.destroy.bind(readable);
    readable.destroy = (error?: unknown) => {
      if (typeof reader.cancel === "function") {
        void reader.cancel(error);
      }
      return originalDestroy(error);
    };

    return readable;
  }

  static toWeb(readable: Readable) {
    if (!readable || typeof readable.on !== "function") {
      throw new TypeError("Readable.toWeb expects a Readable stream instance");
    }

    return new ReadableStream({
      start(controller) {
        const onData = (chunk: unknown) => {
          controller.enqueue(chunk);
        };
        const onEnd = () => {
          controller.close();
          cleanup();
        };
        const onError = (err: unknown) => {
          controller.error(err);
          cleanup();
        };
        const cleanup = () => {
          readable.removeListener("data", onData);
          readable.removeListener("end", onEnd);
          readable.removeListener("error", onError);
        };

        readable.on("data", onData);
        readable.once("end", onEnd);
        readable.once("error", onError);
      },
      cancel(reason) {
        readable.destroy(reason);
      },
    });
  }

  push(chunk: unknown) {
    if (this.destroyed) return false;

    if (chunk === null) {
      this.readableEnded = true;
      this.emit("end");
      this.emit("close");
      return false;
    }

    // If paused or buffer is full, queue the chunk
    if (this.#paused || this.#buffer.length >= this.#highWaterMark) {
      this.#buffer.push(chunk);
      return false; // Signal backpressure
    }

    // Emit data immediately if not paused and buffer is not full
    this.emit("data", chunk);

    // If buffer has accumulated data, it means we hit highWaterMark on previous call
    // Return false to signal backpressure
    return this.#buffer.length === 0;
  }

  pause() {
    this.#paused = true;
    this.emit("pause");
    return this;
  }

  resume() {
    this.#paused = false;
    this.emit("resume");

    // Flush internal buffer while not paused
    while (!this.#paused && this.#buffer.length > 0) {
      const chunk = this.#buffer.shift();
      if (chunk === null) {
        this.readableEnded = true;
        this.emit("end");
        this.emit("close");
        break;
      } else {
        this.emit("data", chunk);
      }
    }

    return this;
  }

  pipe(destination: Writable | Transform | Duplex) {
    const onData = (chunk: unknown) => {
      const canContinue = destination.write(chunk);
      if (!canContinue) {
        this.pause();
      }
    };

    const onDrain = () => {
      this.resume();
    };

    this.on("data", onData);
    this.on("end", () => {
      destination.end();
    });
    this.on("error", (err: unknown) => {
      destination.emit("error", err);
    });

    destination.on("drain", onDrain);

    return destination;
  }

  destroy(error?: unknown) {
    this.destroyed = true;
    if (error !== undefined) {
      this.emit("error", error);
    }
    this.emit("close");
    return this;
  }
}

class Writable extends Stream {
  writable = true;
  destroyed = false;
  writableEnded = false;
  #writeImpl?: (chunk: unknown, encoding: string, cb: Callback) => void;
  #buffer: Array<{ data: unknown; encoding: string; size: number; cb: Callback }> = [];
  #highWaterMark: number;
  #writing = false;
  #bufferedBytes = 0;
  #ending = false;
  #endCallbacks: Array<() => void> = [];

  constructor(options: Record<string, unknown> = {}) {
    super();
    this.#writeImpl = options.write as
      | ((chunk: unknown, encoding: string, cb: Callback) => void)
      | undefined;
    this.#highWaterMark = (options.highWaterMark as number) || 16384;
  }

  static fromWeb(webWritable: WritableStream<unknown>) {
    if (!webWritable || typeof (webWritable as WritableStream<unknown>).getWriter !== "function") {
      throw new TypeError("Writable.fromWeb expects a WritableStream");
    }

    const writer = webWritable.getWriter();

    const writable = new Writable({
      write(chunk: unknown, _encoding: string, cb: Callback) {
        Promise.resolve(writer.write(chunk)).then(
          () => cb(),
          (err) => cb(err),
        );
      },
    });

    writable.once("finish", () => {
      void writer.close();
    });

    const originalDestroy = writable.destroy.bind(writable);
    writable.destroy = (error?: unknown) => {
      void writer.abort(error);
      return originalDestroy(error);
    };

    return writable;
  }

  static toWeb(writable: Writable) {
    if (!writable || typeof writable.write !== "function") {
      throw new TypeError("Writable.toWeb expects a Writable stream instance");
    }

    return new WritableStream({
      write(chunk) {
        return new Promise<void>((resolve, reject) => {
          let settled = false;
          const resolveOnce = () => {
            if (settled) return;
            settled = true;
            resolve();
          };
          const rejectOnce = (err: unknown) => {
            if (settled) return;
            settled = true;
            reject(err);
          };

          const ok = writable.write(chunk, (err?: unknown) => {
            if (err) rejectOnce(err);
            else resolveOnce();
          });

          if (!ok) {
            writable.once("drain", () => resolveOnce());
          }
        });
      },
      close() {
        return new Promise<void>((resolve, reject) => {
          writable.end((err?: unknown) => {
            if (err) reject(err);
            else resolve();
          });
        });
      },
      abort(reason) {
        writable.destroy(reason);
      },
    });
  }

  write(chunk: unknown, encodingOrCb?: string | Callback, maybeCb?: Callback) {
    if (this.destroyed || this.writableEnded) return false;

    const encoding = typeof encodingOrCb === "string" ? encodingOrCb : "utf8";
    const cb = (typeof encodingOrCb === "function" ? encodingOrCb : maybeCb) ?? (() => {});
    const size = byteLengthOfChunk(chunk, encoding);

    const entry = { data: chunk, encoding, size, cb };

    // If already writing, buffer the chunk
    if (this.#writing) {
      this.#buffer.push(entry);
      this.#bufferedBytes += size;
      return this.#bufferedBytes < this.#highWaterMark;
    }

    this.#writing = true;
    this.#performWrite(entry);

    return this.#bufferedBytes < this.#highWaterMark;
  }

  #performWrite(entry: { data: unknown; encoding: string; size: number; cb: Callback }) {
    const done = (err?: unknown) => {
      entry.cb(err);

      if (err !== undefined) {
        this.#writing = false;
        this.emit("error", err);
        return;
      }

      this.#writing = false;

      if (this.#buffer.length > 0) {
        const nextChunk = this.#buffer.shift()!;
        this.#bufferedBytes = Math.max(0, this.#bufferedBytes - nextChunk.size);
        this.#writing = true;
        this.#performWrite(nextChunk);
        return;
      }

      queueMicrotask(() => this.emit("drain"));

      if (this.#ending) {
        this.#finalizeEnd();
      }
    };

    if (this.#writeImpl) {
      this.#writeImpl(entry.data, entry.encoding, done);
    } else {
      done();
    }
  }

  end(chunkOrCb?: unknown, encodingOrCb?: string | Callback, maybeCb?: Callback) {
    if (typeof chunkOrCb === "function") {
      chunkOrCb();
    } else if (chunkOrCb !== undefined) {
      this.write(chunkOrCb, encodingOrCb as string | Callback, maybeCb);
    }

    const cb =
      (typeof encodingOrCb === "function" ? encodingOrCb : maybeCb) ??
      (typeof chunkOrCb === "function" ? chunkOrCb : undefined);

    if (typeof cb === "function") {
      this.#endCallbacks.push(() => cb());
    }

    this.#ending = true;
    if (!this.#writing && this.#buffer.length === 0) {
      this.#finalizeEnd();
    }
    return this;
  }

  #finalizeEnd() {
    if (this.writableEnded) return;
    this.writableEnded = true;
    this.emit("finish");
    this.emit("close");
    for (const cb of this.#endCallbacks) {
      cb();
    }
    this.#endCallbacks = [];
  }

  destroy(error?: unknown) {
    this.destroyed = true;
    if (error !== undefined) {
      this.emit("error", error);
    }
    this.emit("close");
    return this;
  }
}

class Duplex extends Readable {
  writable = true;
  writableEnded = false;
  #writeImpl?: (chunk: unknown, encoding: string, cb: Callback) => void;

  constructor(options: Record<string, unknown> = {}) {
    super(options);
    this.#writeImpl = options.write as
      | ((chunk: unknown, encoding: string, cb: Callback) => void)
      | undefined;
  }

  write(chunk: unknown, encodingOrCb?: string | Callback, maybeCb?: Callback) {
    if (this.writableEnded) return false;

    const encoding = typeof encodingOrCb === "string" ? encodingOrCb : "utf8";
    const cb = (typeof encodingOrCb === "function" ? encodingOrCb : maybeCb) ?? (() => {});

    if (this.#writeImpl) {
      this.#writeImpl(chunk, encoding, cb);
    } else {
      cb();
    }

    return true;
  }

  end(chunkOrCb?: unknown, encodingOrCb?: string | Callback, maybeCb?: Callback) {
    if (typeof chunkOrCb === "function") {
      chunkOrCb();
    } else if (chunkOrCb !== undefined) {
      this.write(chunkOrCb, encodingOrCb as string | Callback, maybeCb);
    }

    const cb =
      (typeof encodingOrCb === "function" ? encodingOrCb : maybeCb) ??
      (typeof chunkOrCb === "function" ? chunkOrCb : undefined);

    this.writableEnded = true;
    this.push(null);
    this.emit("finish");
    if (typeof cb === "function") cb();
    return this;
  }
}

class Transform extends Duplex {
  #transformImpl?: (
    chunk: unknown,
    encoding: string,
    cb: (err?: unknown, data?: unknown) => void,
  ) => void;

  constructor(options: Record<string, unknown> = {}) {
    super(options);
    this.#transformImpl = options.transform as
      | ((chunk: unknown, encoding: string, cb: (err?: unknown, data?: unknown) => void) => void)
      | undefined;
  }

  write(chunk: unknown, encodingOrCb?: string | Callback, maybeCb?: Callback) {
    if (this.writableEnded) return false;

    const encoding = typeof encodingOrCb === "string" ? encodingOrCb : "utf8";
    const cb = (typeof encodingOrCb === "function" ? encodingOrCb : maybeCb) ?? (() => {});

    const done = (err?: unknown, data?: unknown) => {
      if (err !== undefined) {
        this.emit("error", err);
        cb(err);
        return;
      }
      if (data !== undefined && data !== null) {
        this.push(data);
      }
      cb();
    };

    if (this.#transformImpl) {
      this.#transformImpl(chunk, encoding, done);
    } else {
      done(undefined, chunk);
    }

    return true;
  }
}

class PassThrough extends Transform {
  constructor(options: Record<string, unknown> = {}) {
    super({
      ...options,
      transform: (chunk: unknown, _encoding: string, cb: (err?: unknown, data?: unknown) => void) => {
        cb(undefined, chunk);
      },
    });
  }
}

function byteLengthOfChunk(chunk: unknown, encoding: string): number {
  if (chunk === null || chunk === undefined) return 0;
  if (typeof chunk === "string") {
    return Buffer.byteLength(chunk, encoding as BufferEncoding);
  }
  if (chunk instanceof Uint8Array) {
    return chunk.byteLength;
  }
  if (ArrayBuffer.isView(chunk)) {
    return chunk.byteLength;
  }
  if (chunk instanceof ArrayBuffer) {
    return chunk.byteLength;
  }
  return Buffer.byteLength(String(chunk), encoding as BufferEncoding);
}

function pipeline(...streamsOrCb: unknown[]) {
  let options: PipelineOptions | undefined;
  const cb = typeof streamsOrCb[streamsOrCb.length - 1] === "function"
    ? (streamsOrCb.pop() as (err?: unknown) => void)
    : undefined;

  const maybeOptions = streamsOrCb[streamsOrCb.length - 1];
  if (
    maybeOptions &&
    typeof maybeOptions === "object" &&
    "signal" in (maybeOptions as Record<string, unknown>)
  ) {
    options = streamsOrCb.pop() as PipelineOptions;
  }

  const streams = streamsOrCb as Array<Readable | Writable | Transform | Duplex>;

  if (streams.length < 2) {
    if (cb) cb(new Error("pipeline requires at least two streams"));
    return undefined;
  }

  for (let i = 0; i < streams.length - 1; i++) {
    streams[i].pipe(streams[i + 1] as Writable | Transform | Duplex);
  }

  const last = streams[streams.length - 1] as Writable;

  let settled = false;
  const done = (err?: unknown) => {
    if (settled) return;
    settled = true;

    if (signal && onAbort) {
      signal.removeEventListener("abort", onAbort);
    }

    if (cb) cb(err);
  };

  const handleStreamError = (err: unknown) => {
    done(err);
  };

  for (const stream of streams) {
    stream.once("error", handleStreamError);
  }

  const signal = options?.signal;
  const toAbortError = () => {
    const reason = signal?.reason;
    if (reason instanceof Error) {
      return reason;
    }
    return new Error("The operation was aborted");
  };

  const abortAllStreams = (err: Error) => {
    for (const stream of streams) {
      if (typeof (stream as { destroy?: (error?: unknown) => void }).destroy === "function") {
        (stream as { destroy: (error?: unknown) => void }).destroy(err);
      }
    }
  };

  const onAbort = signal
    ? () => {
        const abortErr = toAbortError();
        abortAllStreams(abortErr);
        done(abortErr);
      }
    : undefined;

  if (signal?.aborted) {
    const abortErr = toAbortError();
    abortAllStreams(abortErr);
    done(abortErr);
    return last;
  }

  if (signal && onAbort) {
    signal.addEventListener("abort", onAbort, { once: true });
  }

  if (cb) {
    last.once("finish", () => done());
  }

  return last;
}

function finished(
  stream: Readable | Writable | Duplex | Transform,
  cb: (err?: unknown) => void,
) {
  let done = false;
  const onceDone = (err?: unknown) => {
    if (done) return;
    done = true;
    cb(err);
  };

  stream.once("end", () => onceDone());
  stream.once("finish", () => onceDone());
  stream.once("close", () => onceDone());
  stream.once("error", (err: unknown) => onceDone(err));
}

const promises = {
  pipeline: (...streams: unknown[]) =>
    new Promise<void>((resolve, reject) => {
      pipeline(...streams, (err?: unknown) => {
        if (err) reject(err);
        else resolve();
      });
    }),
};

const streamModule = {
  Stream,
  Readable,
  Writable,
  Duplex,
  Transform,
  PassThrough,
  pipeline,
  finished,
  promises,
};

export {
  Stream,
  Readable,
  Writable,
  Duplex,
  Transform,
  PassThrough,
  pipeline,
  finished,
  promises,
};

export default streamModule;
