type NodeLikeError = Error & { code?: string };

function notImplemented(api: string): never {
  const err = new Error(
    `[edge-runtime] ${api} is not implemented in this runtime profile`,
  ) as NodeLikeError;
  err.code = "ERR_NOT_IMPLEMENTED";
  throw err;
}

function notImplementedAsync(api: string): Promise<never> {
  try {
    notImplemented(api);
  } catch (err) {
    return Promise.reject(err);
  }
  return Promise.reject(new Error("unexpected dns stub state"));
}

function lookup(_hostname: string, cb?: (...args: unknown[]) => void): void {
  if (typeof cb === "function") {
    try {
      notImplemented("dns.lookup");
    } catch (err) {
      cb(err);
    }
  } else {
    notImplemented("dns.lookup");
  }
}

function resolve(): never {
  return notImplemented("dns.resolve");
}

function reverse(): never {
  return notImplemented("dns.reverse");
}

const promises = {
  lookup(hostname: string) {
    return notImplementedAsync(`dns.promises.lookup(${hostname})`);
  },
  resolve(hostname: string) {
    return notImplementedAsync(`dns.promises.resolve(${hostname})`);
  },
  reverse(ip: string) {
    return notImplementedAsync(`dns.promises.reverse(${ip})`);
  },
};

const dnsModule = { lookup, resolve, reverse, promises };

export { lookup, resolve, reverse, promises };
export default dnsModule;
