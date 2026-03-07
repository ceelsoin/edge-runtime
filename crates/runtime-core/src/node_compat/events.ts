class EventEmitter {
  #events = new Map();
  #maxListeners = 10;

  setMaxListeners(n: number) {
    this.#maxListeners = Number(n);
    return this;
  }

  getMaxListeners() {
    return this.#maxListeners;
  }

  emit(eventName: string | symbol, ...args: unknown[]) {
    const listeners = this.#events.get(eventName);
    if (!listeners || listeners.length === 0) return false;

    for (const listener of [...listeners]) {
      listener.apply(this, args);
    }
    return true;
  }

  addListener(eventName: string | symbol, listener: (...args: unknown[]) => void) {
    if (typeof listener !== "function") {
      throw new TypeError("listener must be a function");
    }

    const listeners = this.#events.get(eventName) ?? [];
    listeners.push(listener);
    this.#events.set(eventName, listeners);
    return this;
  }

  on(eventName: string | symbol, listener: (...args: unknown[]) => void) {
    return this.addListener(eventName, listener);
  }

  prependListener(eventName: string | symbol, listener: (...args: unknown[]) => void) {
    if (typeof listener !== "function") {
      throw new TypeError("listener must be a function");
    }

    const listeners = this.#events.get(eventName) ?? [];
    listeners.unshift(listener);
    this.#events.set(eventName, listeners);
    return this;
  }

  once(eventName: string | symbol, listener: (...args: unknown[]) => void) {
    if (typeof listener !== "function") {
      throw new TypeError("listener must be a function");
    }

    const wrapped = (...args: unknown[]) => {
      this.removeListener(eventName, wrapped);
      listener.apply(this, args);
    };

    return this.addListener(eventName, wrapped);
  }

  off(eventName: string | symbol, listener: (...args: unknown[]) => void) {
    return this.removeListener(eventName, listener);
  }

  removeListener(eventName: string | symbol, listener: (...args: unknown[]) => void) {
    const listeners = this.#events.get(eventName);
    if (!listeners || listeners.length === 0) return this;

    const next = listeners.filter((fn) => fn !== listener);
    if (next.length > 0) {
      this.#events.set(eventName, next);
    } else {
      this.#events.delete(eventName);
    }

    return this;
  }

  removeAllListeners(eventName?: string | symbol) {
    if (eventName === undefined) {
      this.#events.clear();
      return this;
    }
    this.#events.delete(eventName);
    return this;
  }

  listenerCount(eventName: string | symbol) {
    return (this.#events.get(eventName) ?? []).length;
  }

  listeners(eventName: string | symbol) {
    return [...(this.#events.get(eventName) ?? [])];
  }

  eventNames() {
    return [...this.#events.keys()];
  }
}

function once(emitter: EventEmitter, eventName: string | symbol): Promise<unknown[]> {
  return new Promise((resolve, reject) => {
    const onEvent = (...args: unknown[]) => {
      emitter.removeListener("error", onError);
      resolve(args);
    };
    const onError = (error: unknown) => {
      emitter.removeListener(eventName, onEvent);
      reject(error);
    };

    emitter.once(eventName, onEvent);
    emitter.once("error", onError);
  });
}

export { EventEmitter, once };

export default {
  EventEmitter,
  once,
};
