class AsyncLocalStorage<T = unknown> {
  #store: T | undefined;

  run<R>(store: T, callback: (...args: unknown[]) => R, ...args: unknown[]): R {
    const prev = this.#store;
    this.#store = store;
    try {
      return callback(...args);
    } finally {
      this.#store = prev;
    }
  }

  enterWith(store: T): void {
    this.#store = store;
  }

  getStore(): T | undefined {
    return this.#store;
  }

  disable(): void {
    this.#store = undefined;
  }
}

class AsyncResource {
  type: string;

  constructor(type: string) {
    this.type = type;
  }

  runInAsyncScope<R>(fn: (...args: unknown[]) => R, thisArg?: unknown, ...args: unknown[]): R {
    return fn.apply(thisArg, args);
  }

  emitDestroy(): void {}
}

function createHook() {
  return {
    enable() {
      return this;
    },
    disable() {
      return this;
    },
  };
}

function executionAsyncId(): number {
  return 0;
}

function triggerAsyncId(): number {
  return 0;
}

const asyncHooks = {
  AsyncLocalStorage,
  AsyncResource,
  createHook,
  executionAsyncId,
  triggerAsyncId,
};

export { AsyncLocalStorage, AsyncResource, createHook, executionAsyncId, triggerAsyncId };
export default asyncHooks;
