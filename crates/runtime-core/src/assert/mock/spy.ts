import { mockFn, type AnyFunction, type Mock } from "./mockFn.ts";

type MethodKey<T extends object> = {
  [K in keyof T]: T[K] extends AnyFunction ? K : never;
}[keyof T];

export type Spy<T extends AnyFunction> = Mock<T> & {
  restore: () => void;
};

export function spyOn<T extends object, K extends MethodKey<T>>(
  target: T,
  key: K,
): Spy<Extract<T[K], AnyFunction>> {
  const original = target[key];

  if (typeof original !== "function") {
    throw new TypeError(`Cannot spy on '${String(key)}': target member is not a function`);
  }

  let restored = false;
  const spy = mockFn(function (this: unknown, ...args: unknown[]) {
    return (original as AnyFunction).apply(this, args);
  } as Extract<T[K], AnyFunction>) as Spy<Extract<T[K], AnyFunction>>;

  (target as Record<PropertyKey, unknown>)[key as PropertyKey] = spy as unknown as T[K];

  spy.restore = () => {
    if (restored) return;
    restored = true;
    (target as Record<PropertyKey, unknown>)[key as PropertyKey] = original as unknown;
  };

  return spy;
}
