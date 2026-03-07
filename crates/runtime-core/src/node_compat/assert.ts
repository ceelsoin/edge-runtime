class AssertionError extends Error {
  code: string;

  constructor(message: string) {
    super(message);
    this.name = "AssertionError";
    this.code = "ERR_ASSERTION";
  }
}

function fail(message = "Assertion failed"): never {
  throw new AssertionError(message);
}

function ok(value: unknown, message = "Expected value to be truthy"): void {
  if (!value) fail(message);
}

function equal(actual: unknown, expected: unknown, message?: string): void {
  if (actual != expected) fail(message ?? `Expected ${actual} == ${expected}`);
}

function strictEqual(actual: unknown, expected: unknown, message?: string): void {
  if (actual !== expected) fail(message ?? `Expected ${actual} === ${expected}`);
}

function notEqual(actual: unknown, expected: unknown, message?: string): void {
  if (actual == expected) fail(message ?? `Expected ${actual} != ${expected}`);
}

function deepEqual(actual: unknown, expected: unknown, message?: string): void {
  const a = JSON.stringify(actual);
  const b = JSON.stringify(expected);
  if (a !== b) fail(message ?? `Expected deepEqual ${a} === ${b}`);
}

function doesNotThrow(fn: () => unknown, message?: string): void {
  try {
    fn();
  } catch (err) {
    fail(message ?? `Expected function not to throw: ${String((err as Error)?.message ?? err)}`);
  }
}

function throws(fn: () => unknown, expected?: RegExp | ((err: unknown) => boolean), message?: string): void {
  let thrown: unknown;
  try {
    fn();
  } catch (err) {
    thrown = err;
  }

  if (thrown === undefined) {
    fail(message ?? "Expected function to throw");
  }

  if (expected instanceof RegExp) {
    const msg = String((thrown as Error)?.message ?? thrown);
    if (!expected.test(msg)) {
      fail(message ?? `Thrown message '${msg}' does not match ${expected}`);
    }
  } else if (typeof expected === "function") {
    if (!expected(thrown)) {
      fail(message ?? "Thrown error does not satisfy predicate");
    }
  }
}

const assertFn = ((value: unknown, message?: string) => ok(value, message)) as typeof ok & {
  ok: typeof ok;
  equal: typeof equal;
  strictEqual: typeof strictEqual;
  notEqual: typeof notEqual;
  deepEqual: typeof deepEqual;
  doesNotThrow: typeof doesNotThrow;
  throws: typeof throws;
  fail: typeof fail;
  AssertionError: typeof AssertionError;
};

assertFn.ok = ok;
assertFn.equal = equal;
assertFn.strictEqual = strictEqual;
assertFn.notEqual = notEqual;
assertFn.deepEqual = deepEqual;
assertFn.doesNotThrow = doesNotThrow;
assertFn.throws = throws;
assertFn.fail = fail;
assertFn.AssertionError = AssertionError;

export {
  AssertionError,
  ok,
  equal,
  strictEqual,
  notEqual,
  deepEqual,
  doesNotThrow,
  throws,
  fail,
};

export default assertFn;
