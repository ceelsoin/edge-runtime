function setTimeoutPromise(delay = 0, value?: unknown, options?: { signal?: AbortSignal }) {
  return new Promise((resolve, reject) => {
    const signal = options?.signal;
    if (signal?.aborted) {
      reject(new Error("Aborted"));
      return;
    }

    const id = globalThis.setTimeout(() => {
      resolve(value);
    }, Number(delay));

    signal?.addEventListener(
      "abort",
      () => {
        globalThis.clearTimeout(id);
        reject(new Error("Aborted"));
      },
      { once: true },
    );
  });
}

function setImmediatePromise(value?: unknown) {
  return new Promise((resolve) => {
    if (typeof globalThis.setImmediate === "function") {
      globalThis.setImmediate(() => resolve(value));
      return;
    }
    queueMicrotask(() => resolve(value));
  });
}

function setIntervalPromise(delay = 0, value?: unknown, options?: { signal?: AbortSignal }) {
  const signal = options?.signal;
  async function* iterator() {
    while (!signal?.aborted) {
      await setTimeoutPromise(delay, undefined, options);
      if (signal?.aborted) break;
      yield value;
    }
  }
  return iterator();
}

const timersPromises = {
  setTimeout: setTimeoutPromise,
  setImmediate: setImmediatePromise,
  setInterval: setIntervalPromise,
};

export {
  setTimeoutPromise as setTimeout,
  setImmediatePromise as setImmediate,
  setIntervalPromise as setInterval,
};

export default timersPromises;
