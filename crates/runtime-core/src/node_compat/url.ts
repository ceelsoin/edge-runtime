function getURLCtor(): typeof globalThis.URL {
  const ctor = globalThis.URL;
  if (typeof ctor !== "function") {
    throw new TypeError("URL constructor is not available in this runtime context");
  }
  return ctor;
}

function getURLSearchParamsCtor(): typeof globalThis.URLSearchParams {
  const ctor = globalThis.URLSearchParams;
  if (typeof ctor !== "function") {
    throw new TypeError("URLSearchParams constructor is not available in this runtime context");
  }
  return ctor;
}

// Delay global URL binding so imports remain stable even during early bootstrap.
class URLCompat {
  constructor(input: string, base?: string | URL) {
    return new (getURLCtor())(input, base);
  }

  static canParse(input: string, base?: string | URL): boolean {
    const ctor = getURLCtor() as typeof URL & { canParse?: (u: string, b?: string | URL) => boolean };
    if (typeof ctor.canParse === "function") {
      return ctor.canParse(input, base);
    }
    try {
      // eslint-disable-next-line no-new
      new ctor(input, base);
      return true;
    } catch {
      return false;
    }
  }

  static parse(input: string, base?: string | URL): URL | null {
    try {
      return new (getURLCtor())(input, base);
    } catch {
      return null;
    }
  }
}

class URLSearchParamsCompat {
  constructor(init?: string | URLSearchParams | Record<string, string> | string[][]) {
    return new (getURLSearchParamsCtor())(init as ConstructorParameters<typeof URLSearchParams>[0]);
  }
}

function toPathString(input: string): string {
  const value = String(input);
  if (!value.startsWith("file://")) {
    throw new TypeError("Only file: URLs can be converted to paths");
  }
  const pathPart = value.slice("file://".length);
  return decodeURIComponent(pathPart || "/");
}

function fileURLToPath(input: string | URL): string {
  return toPathString(input instanceof URL ? input.toString() : String(input));
}

function pathToFileURL(path: string): URL {
  const normalized = String(path).startsWith("/") ? String(path) : `/${String(path)}`;
  const href = `file://${encodeURI(normalized)}`;
  return new (getURLCtor())(href);
}

function domainToASCII(domain: string): string {
  try {
    return new (getURLCtor())(`http://${String(domain)}`).hostname;
  } catch {
    return "";
  }
}

function domainToUnicode(domain: string): string {
  try {
    const hostname = String(domain).trim();
    if (!hostname) return "";
    // Use IDNA decoding via URL host parsing when possible.
    return new (getURLCtor())(`http://${hostname}`).hostname;
  } catch {
    return "";
  }
}

const URLPattern = (globalThis as { URLPattern?: unknown }).URLPattern;

const urlModule = {
  URL: URLCompat,
  URLSearchParams: URLSearchParamsCompat,
  URLPattern,
  fileURLToPath,
  pathToFileURL,
  domainToASCII,
  domainToUnicode,
};

export const URL = URLCompat as unknown as typeof globalThis.URL;
export const URLSearchParams = URLSearchParamsCompat as unknown as typeof globalThis.URLSearchParams;
export { URLPattern, fileURLToPath, pathToFileURL, domainToASCII, domainToUnicode };

export default urlModule;
