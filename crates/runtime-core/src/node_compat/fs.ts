type FsError = Error & {
  code: string;
  errno: number;
  syscall?: string;
  path?: string;
};

function fsNotSupported(syscall: string, path?: string): never {
  const err = new Error(
    `[edge-runtime] ${syscall} is not supported: sem acesso real ao FS neste runtime`,
  ) as FsError;
  err.name = "Error";
  err.code = "EOPNOTSUPP";
  err.errno = 95;
  err.syscall = syscall;
  if (path !== undefined) err.path = path;
  throw err;
}

function asPath(path: unknown): string {
  if (typeof path === "string") return path;
  if (path instanceof URL) return path.toString();
  return String(path);
}

const constants = Object.freeze({
  F_OK: 0,
  R_OK: 4,
  W_OK: 2,
  X_OK: 1,
});

function existsSync(_path: unknown): boolean {
  // Feature-detection friendly: no real filesystem exposure.
  return false;
}

function accessSync(path: unknown): void {
  fsNotSupported("access", asPath(path));
}

function readFileSync(path: unknown): never {
  fsNotSupported("readFile", asPath(path));
}

function writeFileSync(path: unknown): never {
  fsNotSupported("writeFile", asPath(path));
}

function statSync(path: unknown): never {
  fsNotSupported("stat", asPath(path));
}

function lstatSync(path: unknown): never {
  fsNotSupported("lstat", asPath(path));
}

function readdirSync(path: unknown): never {
  fsNotSupported("readdir", asPath(path));
}

function createReadStream(path: unknown): never {
  fsNotSupported("createReadStream", asPath(path));
}

function createWriteStream(path: unknown): never {
  fsNotSupported("createWriteStream", asPath(path));
}

function watch(path: unknown): never {
  fsNotSupported("watch", asPath(path));
}

function callbackStyle(op: string, path: unknown, cb?: (...args: unknown[]) => void): void {
  const callback = typeof cb === "function" ? cb : undefined;
  const err = (() => {
    try {
      fsNotSupported(op, asPath(path));
      return undefined;
    } catch (e) {
      return e;
    }
  })();
  if (callback) callback(err);
}

function readFile(path: unknown, _options?: unknown, cb?: (...args: unknown[]) => void): void {
  callbackStyle("readFile", path, cb);
}

function writeFile(
  path: unknown,
  _data?: unknown,
  _options?: unknown,
  cb?: (...args: unknown[]) => void,
): void {
  callbackStyle("writeFile", path, cb);
}

function stat(path: unknown, cb?: (...args: unknown[]) => void): void {
  callbackStyle("stat", path, cb);
}

function lstat(path: unknown, cb?: (...args: unknown[]) => void): void {
  callbackStyle("lstat", path, cb);
}

function readdir(path: unknown, cb?: (...args: unknown[]) => void): void {
  callbackStyle("readdir", path, cb);
}

const fsModule = {
  constants,
  existsSync,
  accessSync,
  readFileSync,
  writeFileSync,
  statSync,
  lstatSync,
  readdirSync,
  createReadStream,
  createWriteStream,
  watch,
  readFile,
  writeFile,
  stat,
  lstat,
  readdir,
};

export {
  constants,
  existsSync,
  accessSync,
  readFileSync,
  writeFileSync,
  statSync,
  lstatSync,
  readdirSync,
  createReadStream,
  createWriteStream,
  watch,
  readFile,
  writeFile,
  stat,
  lstat,
  readdir,
};

export default fsModule;
