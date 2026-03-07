function setTimeoutCompat(...args: unknown[]) {
  return globalThis.setTimeout(...(args as Parameters<typeof globalThis.setTimeout>));
}

function clearTimeoutCompat(...args: unknown[]) {
  return globalThis.clearTimeout(...(args as Parameters<typeof globalThis.clearTimeout>));
}

function setIntervalCompat(...args: unknown[]) {
  return globalThis.setInterval(...(args as Parameters<typeof globalThis.setInterval>));
}

function clearIntervalCompat(...args: unknown[]) {
  return globalThis.clearInterval(...(args as Parameters<typeof globalThis.clearInterval>));
}

function setImmediateCompat(...args: unknown[]) {
  if (typeof globalThis.setImmediate === "function") {
    return globalThis.setImmediate(...(args as [TimerHandler, ...unknown[]]));
  }
  const callback = args[0];
  const rest = args.slice(1);
  queueMicrotask(() => {
    if (typeof callback === "function") {
      callback(...rest);
    }
  });
  return 0;
}

function clearImmediateCompat(id: number): void {
  if (typeof globalThis.clearImmediate === "function") {
    globalThis.clearImmediate(id);
  }
}

function queueMicrotaskCompat(cb: VoidFunction): void {
  globalThis.queueMicrotask(cb);
}

const timers = {
  setTimeout: setTimeoutCompat,
  clearTimeout: clearTimeoutCompat,
  setInterval: setIntervalCompat,
  clearInterval: clearIntervalCompat,
  setImmediate: setImmediateCompat,
  clearImmediate: clearImmediateCompat,
  queueMicrotask: queueMicrotaskCompat,
};

export const setTimeout = setTimeoutCompat;
export const clearTimeout = clearTimeoutCompat;
export const setInterval = setIntervalCompat;
export const clearInterval = clearIntervalCompat;
export const setImmediate = setImmediateCompat;
export const clearImmediate = clearImmediateCompat;
export const queueMicrotask = queueMicrotaskCompat;

export default timers;
