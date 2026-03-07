const kCustomInspect = Symbol.for("nodejs.util.inspect.custom");
const kPromisifyCustom = Symbol.for("nodejs.util.promisify.custom");

function formatValue(value: unknown): string {
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "bigint" || typeof value === "boolean") {
    return String(value);
  }
  if (typeof value === "undefined") return "undefined";
  if (value === null) return "null";
  if (typeof value === "symbol") return value.toString();
  if (typeof value === "function") return `[Function: ${value.name || "anonymous"}]`;

  try {
    return JSON.stringify(value);
  } catch {
    return Object.prototype.toString.call(value);
  }
}

function format(fmt: unknown, ...args: unknown[]): string {
  if (typeof fmt !== "string") {
    return [fmt, ...args].map(formatValue).join(" ");
  }

  let argIndex = 0;
  const out = fmt.replace(/%[sdifoOj%]/g, (token) => {
    if (token === "%%") return "%";
    const value = args[argIndex++];
    switch (token) {
      case "%s":
        return String(value);
      case "%d":
      case "%i":
        return Number(value).toString();
      case "%f":
        return Number(value).toString();
      case "%o":
      case "%O":
      case "%j":
        return formatValue(value);
      default:
        return token;
    }
  });

  if (argIndex < args.length) {
    return `${out} ${args.slice(argIndex).map(formatValue).join(" ")}`;
  }

  return out;
}

function inspect(value: unknown): string {
  if (value && typeof value === "object") {
    const maybeCustom = (value as Record<PropertyKey, unknown>)[kCustomInspect];
    if (typeof maybeCustom === "function") {
      try {
        return String((maybeCustom as (...args: unknown[]) => unknown).call(value));
      } catch {
        // Fall back to default formatting.
      }
    }
  }

  return formatValue(value);
}

function inherits(ctor: Function, superCtor: Function) {
  if (typeof ctor !== "function" || typeof superCtor !== "function") {
    throw new TypeError("inherits expects constructor functions");
  }
  Object.setPrototypeOf(ctor.prototype, superCtor.prototype);
  Object.setPrototypeOf(ctor, superCtor);
}

function deprecate<T extends (...args: unknown[]) => unknown>(
  fn: T,
  _message: string,
): T {
  return fn;
}

function callbackify(fn: (...args: unknown[]) => Promise<unknown>) {
  return (...args: unknown[]) => {
    const cb = args.pop();
    if (typeof cb !== "function") {
      throw new TypeError("The last argument must be a callback function");
    }
    Promise.resolve(fn(...args)).then(
      (value) => (cb as (err: unknown, value?: unknown) => void)(null, value),
      (err) => (cb as (err: unknown) => void)(err),
    );
  };
}

function promisify(fn: (...args: unknown[]) => unknown) {
  const custom = (fn as Record<PropertyKey, unknown>)[kPromisifyCustom];
  if (typeof custom === "function") {
    return custom;
  }

  return (...args: unknown[]) =>
    new Promise((resolve, reject) => {
      fn(...args, (err: unknown, value: unknown) => {
        if (err) {
          reject(err);
          return;
        }
        resolve(value);
      });
    });
}

(promisify as Record<PropertyKey, unknown>).custom = kPromisifyCustom;

function isObjectLike(value: unknown) {
  return typeof value === "object" && value !== null;
}

const types = {
  isArrayBufferView(value: unknown) {
    return ArrayBuffer.isView(value);
  },
  isUint8Array(value: unknown) {
    return value instanceof Uint8Array;
  },
  isDate(value: unknown) {
    return value instanceof Date;
  },
  isRegExp(value: unknown) {
    return value instanceof RegExp;
  },
  isPromise(value: unknown) {
    return isObjectLike(value) && typeof (value as Promise<unknown>).then === "function";
  },
  isNativeError(value: unknown) {
    return value instanceof Error;
  },
};

const utilModule = {
  format,
  inspect,
  inherits,
  deprecate,
  callbackify,
  promisify,
  types,
};

export {
  format,
  inspect,
  inherits,
  deprecate,
  callbackify,
  promisify,
  types,
};

export default utilModule;
