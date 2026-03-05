type TimerEntry = {
  id: number;
  runAt: number;
  callback: (...args: unknown[]) => void;
  interval?: number;
  args: unknown[];
  cleared: boolean;
};

export type MockClock = {
  now: () => number;
  tick: (ms: number) => void;
  restore: () => void;
};

function sortByRunAt(a: TimerEntry, b: TimerEntry): number {
  if (a.runAt === b.runAt) {
    return a.id - b.id;
  }
  return a.runAt - b.runAt;
}

export function mockTime(): MockClock {
  const globalScope = globalThis as typeof globalThis & {
    setTimeout: typeof setTimeout;
    clearTimeout: typeof clearTimeout;
    setInterval: typeof setInterval;
    clearInterval: typeof clearInterval;
  };

  const originalSetTimeout = globalScope.setTimeout;
  const originalClearTimeout = globalScope.clearTimeout;
  const originalSetInterval = globalScope.setInterval;
  const originalClearInterval = globalScope.clearInterval;

  let restored = false;
  let now = Date.now();
  let nextId = 1;
  const timers = new Map<number, TimerEntry>();

  const clearTimer = (id?: number) => {
    if (typeof id !== "number") return;
    const timer = timers.get(id);
    if (!timer) return;
    timer.cleared = true;
    timers.delete(id);
  };

  const scheduleTimer = (
    callback: (...args: unknown[]) => void,
    delay: number,
    interval: number | undefined,
    args: unknown[],
  ): number => {
    const id = nextId;
    nextId += 1;

    const safeDelay = Number.isFinite(delay) ? Math.max(0, delay) : 0;
    timers.set(id, {
      id,
      runAt: now + safeDelay,
      callback,
      interval,
      args,
      cleared: false,
    });
    return id;
  };

  const runDueTimers = () => {
    while (true) {
      const due = [...timers.values()]
        .filter((timer) => !timer.cleared && timer.runAt <= now)
        .sort(sortByRunAt)[0];

      if (!due) {
        break;
      }

      if (due.interval === undefined) {
        timers.delete(due.id);
      }

      due.callback(...due.args);

      if (!due.cleared && due.interval !== undefined) {
        due.runAt += due.interval;
      } else {
        timers.delete(due.id);
      }
    }
  };

  globalScope.setTimeout = ((
    callback: TimerHandler,
    delay?: number,
    ...args: unknown[]
  ): number => {
    if (typeof callback !== "function") {
      throw new TypeError("mockTime only supports function callbacks for setTimeout");
    }
    return scheduleTimer(callback as (...args: unknown[]) => void, delay ?? 0, undefined, args);
  }) as typeof setTimeout;

  globalScope.clearTimeout = ((id?: number) => {
    clearTimer(id);
  }) as typeof clearTimeout;

  globalScope.setInterval = ((
    callback: TimerHandler,
    delay?: number,
    ...args: unknown[]
  ): number => {
    if (typeof callback !== "function") {
      throw new TypeError("mockTime only supports function callbacks for setInterval");
    }
    const safeDelay = Number.isFinite(delay ?? 0) ? Math.max(0, delay ?? 0) : 0;
    return scheduleTimer(callback as (...args: unknown[]) => void, safeDelay, safeDelay, args);
  }) as typeof setInterval;

  globalScope.clearInterval = ((id?: number) => {
    clearTimer(id);
  }) as typeof clearInterval;

  return {
    now: () => now,
    tick: (ms: number) => {
      if (!Number.isFinite(ms) || ms < 0) {
        throw new TypeError("tick(ms) requires a non-negative finite number");
      }
      now += ms;
      runDueTimers();
    },
    restore: () => {
      if (restored) return;
      restored = true;
      globalScope.setTimeout = originalSetTimeout;
      globalScope.clearTimeout = originalClearTimeout;
      globalScope.setInterval = originalSetInterval;
      globalScope.clearInterval = originalClearInterval;
      timers.clear();
    },
  };
}
