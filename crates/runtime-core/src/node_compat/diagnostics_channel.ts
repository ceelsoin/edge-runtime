type Handler = (message: unknown, name: string) => void;

class Channel {
  name: string;
  #subscribers = new Set<Handler>();

  constructor(name: string) {
    this.name = name;
  }

  publish(message: unknown): void {
    for (const fn of this.#subscribers) {
      fn(message, this.name);
    }
  }

  subscribe(fn: Handler): void {
    if (typeof fn === "function") this.#subscribers.add(fn);
  }

  unsubscribe(fn: Handler): void {
    this.#subscribers.delete(fn);
  }

  hasSubscribers(): boolean {
    return this.#subscribers.size > 0;
  }
}

const registry = new Map<string, Channel>();

function channel(name: string): Channel {
  const key = String(name);
  const existing = registry.get(key);
  if (existing) return existing;
  const created = new Channel(key);
  registry.set(key, created);
  return created;
}

function hasSubscribers(name: string): boolean {
  return channel(name).hasSubscribers();
}

const diagnosticsChannel = { channel, hasSubscribers, Channel };

export { channel, hasSubscribers, Channel };
export default diagnosticsChannel;
