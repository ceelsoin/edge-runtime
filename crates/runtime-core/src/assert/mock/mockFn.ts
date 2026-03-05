export type AnyFunction = (this: unknown, ...args: any[]) => any;

export type MockCall = {
  args: unknown[];
  result?: unknown;
  error?: unknown;
};

export type Mock<T extends AnyFunction> = ((...args: Parameters<T>) => ReturnType<T>) & {
  calls: MockCall[];
  mockClear: () => void;
  mockImplementation: (nextImpl: T) => void;
};

function isPromiseLike(value: unknown): value is Promise<unknown> {
  return typeof value === "object" && value !== null && typeof (value as Promise<unknown>).then === "function";
}

export function mockFn<T extends AnyFunction>(impl?: T): Mock<T> {
  let implementation = impl ?? ((() => undefined) as T);
  const calls: MockCall[] = [];

  const mock = function (this: unknown, ...args: Parameters<T>): ReturnType<T> {
    const call: MockCall = { args: [...args] };
    calls.push(call);

    try {
      const value = implementation.apply(this, args) as ReturnType<T>;

      if (isPromiseLike(value)) {
        const wrapped = value
          .then((resolved: unknown) => {
            call.result = resolved;
            return resolved;
          })
          .catch((error: unknown) => {
            call.error = error;
            throw error;
          });
        return wrapped as ReturnType<T>;
      }

      call.result = value;
      return value;
    } catch (error) {
      call.error = error;
      throw error;
    }
  } as Mock<T>;

  mock.calls = calls;
  mock.mockClear = () => {
    calls.length = 0;
  };
  mock.mockImplementation = (nextImpl: T) => {
    implementation = nextImpl;
  };

  return mock;
}
