import { EventEmitter } from "node:events";

type Callback = (err?: unknown, value?: unknown) => void;

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
  #buffer: unknown[] = [];
  #highWaterMark: number;
  #writing = false;

  constructor(options: Record<string, unknown> = {}) {
    super();
    this.#writeImpl = options.write as
      | ((chunk: unknown, encoding: string, cb: Callback) => void)
      | undefined;
    this.#highWaterMark = (options.highWaterMark as number) || 16384;
  }

  write(chunk: unknown, encodingOrCb?: string | Callback, maybeCb?: Callback) {
    if (this.destroyed || this.writableEnded) return false;

    const encoding = typeof encodingOrCb === "string" ? encodingOrCb : "utf8";
    const cb = (typeof encodingOrCb === "function" ? encodingOrCb : maybeCb) ?? (() => {});

    const done = () => {
      cb();
      this.#writing = false;

      // Flush buffer if there's more data
      if (this.#buffer.length > 0) {
        const nextChunk = this.#buffer.shift();
        const nextEncoding = typeof nextChunk === 'object' && nextChunk !== null && 'encoding' in nextChunk
          ? (nextChunk as any).encoding
          : 'utf8';
        const nextData = typeof nextChunk === 'object' && nextChunk !== null && 'data' in nextChunk
          ? (nextChunk as any).data
          : nextChunk;
        this.write(nextData, nextEncoding);
      }

      // Emit drain when buffer is flushed
      if (this.#buffer.length === 0) {
        queueMicrotask(() => this.emit("drain"));
      }
    };

    // If already writing, buffer the chunk
    if (this.#writing || this.#buffer.length > 0) {
      this.#buffer.push(chunk);
      return this.#buffer.length < this.#highWaterMark;
    }

    this.#writing = true;

    if (this.#writeImpl) {
      this.#writeImpl(chunk, encoding, done);
    } else {
      done();
    }

    return this.#buffer.length === 0;
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
    this.emit("finish");
    this.emit("close");

    if (typeof cb === "function") cb();
    return this;
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

function pipeline(...streamsOrCb: unknown[]) {
  const cb = typeof streamsOrCb[streamsOrCb.length - 1] === "function"
    ? (streamsOrCb.pop() as (err?: unknown) => void)
    : undefined;

  const streams = streamsOrCb as Array<Readable | Writable | Transform | Duplex>;

  if (streams.length < 2) {
    if (cb) cb(new Error("pipeline requires at least two streams"));
    return undefined;
  }

  for (let i = 0; i < streams.length - 1; i++) {
    streams[i].pipe(streams[i + 1] as Writable | Transform | Duplex);
  }

  const last = streams[streams.length - 1] as Writable;
  if (cb) {
    last.once("finish", () => cb());
    last.once("error", (err: unknown) => cb(err));
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
