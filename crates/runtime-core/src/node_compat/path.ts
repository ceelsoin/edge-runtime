function ensureString(value: unknown, label: string): string {
  if (typeof value !== "string") {
    throw new TypeError(`${label} must be a string`);
  }
  return value;
}

function splitSegments(path: string): string[] {
  return path.split("/").filter((part) => part.length > 0);
}

function normalizeSlashes(path: string): string {
  return path.replace(/\\+/g, "/");
}

function normalize(path: string): string {
  const input = normalizeSlashes(ensureString(path, "path"));
  const isAbs = input.startsWith("/");
  const parts = splitSegments(input);
  const stack: string[] = [];

  for (const part of parts) {
    if (part === ".") continue;
    if (part === "..") {
      if (stack.length > 0 && stack[stack.length - 1] !== "..") {
        stack.pop();
      } else if (!isAbs) {
        stack.push("..");
      }
      continue;
    }
    stack.push(part);
  }

  const out = `${isAbs ? "/" : ""}${stack.join("/")}`;
  if (!out) return isAbs ? "/" : ".";
  return out;
}

function isAbsolute(path: string): boolean {
  return normalizeSlashes(ensureString(path, "path")).startsWith("/");
}

function join(...parts: string[]): string {
  if (parts.length === 0) return ".";
  return normalize(parts.map((p) => ensureString(p, "path segment")).join("/"));
}

function resolve(...parts: string[]): string {
  let resolved = "";
  for (const part of parts) {
    const clean = normalizeSlashes(ensureString(part, "path segment"));
    if (!clean) continue;
    if (clean.startsWith("/")) {
      resolved = clean;
    } else {
      resolved = resolved ? `${resolved}/${clean}` : clean;
    }
  }
  if (!resolved.startsWith("/")) {
    resolved = `/${resolved}`;
  }
  return normalize(resolved);
}

function dirname(path: string): string {
  const normalized = normalize(ensureString(path, "path"));
  if (normalized === "/") return "/";
  const last = normalized.lastIndexOf("/");
  if (last <= 0) return ".";
  return normalized.slice(0, last);
}

function basename(path: string, ext?: string): string {
  const normalized = normalize(ensureString(path, "path"));
  const idx = normalized.lastIndexOf("/");
  let base = idx >= 0 ? normalized.slice(idx + 1) : normalized;
  if (ext && base.endsWith(ext)) {
    base = base.slice(0, Math.max(0, base.length - ext.length));
  }
  return base;
}

function extname(path: string): string {
  const base = basename(path);
  const dot = base.lastIndexOf(".");
  if (dot <= 0) return "";
  return base.slice(dot);
}

function parse(path: string) {
  const normalized = normalize(ensureString(path, "path"));
  const root = normalized.startsWith("/") ? "/" : "";
  const dir = dirname(normalized);
  const base = basename(normalized);
  const ext = extname(base);
  const name = ext ? base.slice(0, base.length - ext.length) : base;
  return { root, dir, base, ext, name };
}

function format(pathObject: {
  root?: string;
  dir?: string;
  base?: string;
  name?: string;
  ext?: string;
}): string {
  const dir = pathObject.dir ?? pathObject.root ?? "";
  const base = pathObject.base ?? `${pathObject.name ?? ""}${pathObject.ext ?? ""}`;
  if (!dir) return base || ".";
  return normalize(`${dir}/${base}`);
}

function relative(from: string, to: string): string {
  const fromNorm = normalize(resolve(from));
  const toNorm = normalize(resolve(to));

  if (fromNorm === toNorm) return "";

  const fromParts = splitSegments(fromNorm);
  const toParts = splitSegments(toNorm);

  let i = 0;
  while (i < fromParts.length && i < toParts.length && fromParts[i] === toParts[i]) {
    i++;
  }

  const up = fromParts.slice(i).map(() => "..");
  const down = toParts.slice(i);
  const out = [...up, ...down].join("/");
  return out || "";
}

function toNamespacedPath(path: string): string {
  return ensureString(path, "path");
}

const sep = "/";
const delimiter = ":";

const posix = {
  sep,
  delimiter,
  normalize,
  isAbsolute,
  join,
  resolve,
  dirname,
  basename,
  extname,
  parse,
  format,
  relative,
  toNamespacedPath,
};

const win32 = {
  ...posix,
  sep: "\\",
  delimiter: ";",
};

const pathModule = {
  ...posix,
  posix,
  win32,
};

export {
  sep,
  delimiter,
  normalize,
  isAbsolute,
  join,
  resolve,
  dirname,
  basename,
  extname,
  parse,
  format,
  relative,
  toNamespacedPath,
  posix,
  win32,
};

export default pathModule;
