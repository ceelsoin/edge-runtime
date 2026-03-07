import http from "node:http";
import https from "node:https";

type HeadersLike = Record<string, string>;
type RequestCallback = (err: unknown, response?: unknown, body?: unknown) => void;
type RequestOptions = {
  url?: string;
  uri?: string;
  method?: string;
  headers?: HeadersLike;
  body?: unknown;
  json?: unknown;
  qs?: Record<string, string | number | boolean | null | undefined>;
};

type RequestLike = {
  on: (event: string, listener: (...args: unknown[]) => void) => RequestLike;
  end: (chunk?: unknown) => RequestLike;
  write: (chunk: unknown) => boolean;
  setHeader: (name: string, value: string) => void;
  getHeader: (name: string) => string | undefined;
  abort: () => void;
};

function withQuery(baseUrl: string, qs?: RequestOptions["qs"]): string {
  if (!qs || Object.keys(qs).length === 0) return baseUrl;
  const url = new URL(baseUrl);
  for (const [key, value] of Object.entries(qs)) {
    if (value === undefined || value === null) continue;
    url.searchParams.set(key, String(value));
  }
  return url.toString();
}

function normalizeInput(input: string | RequestOptions, options?: RequestOptions): RequestOptions {
  if (typeof input === "string") {
    return { ...(options ?? {}), url: options?.url ?? options?.uri ?? input };
  }
  return { ...input, ...(options ?? {}) };
}

function normalizeBody(options: RequestOptions): string | undefined {
  if (options.json !== undefined) {
    if (!options.headers) options.headers = {};
    if (!options.headers["content-type"]) {
      options.headers["content-type"] = "application/json";
    }
    return JSON.stringify(options.json);
  }
  if (options.body === undefined) return undefined;
  if (typeof options.body === "string") return options.body;
  return JSON.stringify(options.body);
}

function requestImpl(
  input: string | RequestOptions,
  optionsOrCb?: RequestOptions | RequestCallback,
  maybeCb?: RequestCallback,
): RequestLike {
  const options =
    typeof optionsOrCb === "object" && optionsOrCb !== null
      ? normalizeInput(input, optionsOrCb)
      : normalizeInput(input, undefined);
  const callback =
    (typeof optionsOrCb === "function" ? optionsOrCb : maybeCb) ?? (() => {});

  const rawUrl = options.url ?? options.uri;
  if (!rawUrl) {
    throw new TypeError("request adapter requires url/uri");
  }

  const url = withQuery(rawUrl, options.qs);
  const method = (options.method ?? "GET").toUpperCase();
  const headers = { ...(options.headers ?? {}) };
  const body = normalizeBody({ ...options, headers });

  const transport = url.startsWith("https:") ? https : http;
  let completed = false;
  let aborted = false;
  let finished = false;
  let callbackCalled = false;

  const done = (err: unknown, response?: unknown, responseBody?: unknown) => {
    if (callbackCalled) return;
    callbackCalled = true;
    callback(err, response, responseBody);
  };

  const req = transport.request(
    url,
    {
      method,
      headers,
    },
    (res: any) => {
      let chunks = "";
      res.on("data", (chunk: unknown) => {
        chunks += String(chunk ?? "");
      });
      res.on("end", () => {
        if (aborted) return;
        completed = true;
        done(null, res, chunks);
      });
      res.on("error", (err: unknown) => {
        if (aborted) return;
        done(err);
      });
    },
  ) as RequestLike;

  const originalWrite = req.write.bind(req);
  const originalEnd = req.end.bind(req);

  req.write = (chunk: unknown) => {
    if (finished || aborted) return false;
    return originalWrite(chunk);
  };

  req.end = (chunk?: unknown) => {
    if (finished || aborted) return req;
    finished = true;
    return originalEnd(chunk);
  };

  const originalAbort = req.abort ? req.abort.bind(req) : undefined;
  req.abort = () => {
    if (completed || aborted) return;
    aborted = true;
    finished = true;
    if (typeof originalAbort === "function") {
      originalAbort();
      return;
    }
    req.on("error", () => {});
  };

  req.on("error", (err: unknown) => {
    if (aborted) return;
    done(err);
  });

  if (body !== undefined) {
    req.write(body);
    req.end();
  } else {
    // Keep `request(url, cb)` ergonomic by dispatching in next microtask,
    // while still allowing immediate `req.write()` / `req.end()` overrides.
    queueMicrotask(() => {
      if (aborted || finished) return;
      req.end();
    });
  }

  return req;
}

const request = requestImpl as typeof requestImpl & {
  get: typeof requestImpl;
  post: typeof requestImpl;
  put: typeof requestImpl;
  patch: typeof requestImpl;
  del: typeof requestImpl;
  delete: typeof requestImpl;
  defaults: (base: RequestOptions) => typeof requestImpl;
};

request.get = (input, optionsOrCb, maybeCb) => {
  const options =
    typeof optionsOrCb === "object" && optionsOrCb !== null
      ? { ...optionsOrCb, method: "GET" }
      : { method: "GET" };
  return requestImpl(input, options, typeof optionsOrCb === "function" ? optionsOrCb : maybeCb);
};

request.post = (input, optionsOrCb, maybeCb) => {
  const options =
    typeof optionsOrCb === "object" && optionsOrCb !== null
      ? { ...optionsOrCb, method: "POST" }
      : { method: "POST" };
  return requestImpl(input, options, typeof optionsOrCb === "function" ? optionsOrCb : maybeCb);
};

request.put = (input, optionsOrCb, maybeCb) => {
  const options =
    typeof optionsOrCb === "object" && optionsOrCb !== null
      ? { ...optionsOrCb, method: "PUT" }
      : { method: "PUT" };
  return requestImpl(input, options, typeof optionsOrCb === "function" ? optionsOrCb : maybeCb);
};

request.patch = (input, optionsOrCb, maybeCb) => {
  const options =
    typeof optionsOrCb === "object" && optionsOrCb !== null
      ? { ...optionsOrCb, method: "PATCH" }
      : { method: "PATCH" };
  return requestImpl(input, options, typeof optionsOrCb === "function" ? optionsOrCb : maybeCb);
};

request.del = (input, optionsOrCb, maybeCb) => {
  const options =
    typeof optionsOrCb === "object" && optionsOrCb !== null
      ? { ...optionsOrCb, method: "DELETE" }
      : { method: "DELETE" };
  return requestImpl(input, options, typeof optionsOrCb === "function" ? optionsOrCb : maybeCb);
};

request.delete = request.del;

request.defaults = (base: RequestOptions) => {
  return (input: string | RequestOptions, optionsOrCb?: RequestOptions | RequestCallback, maybeCb?: RequestCallback) => {
    const mergedInput = typeof input === "string" ? { ...base, url: input } : { ...base, ...input };
    if (typeof optionsOrCb === "function") {
      return requestImpl(mergedInput, optionsOrCb);
    }
    return requestImpl(mergedInput, optionsOrCb, maybeCb);
  };
};

export default request;
