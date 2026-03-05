# Testing API Reference

Import all APIs from:

```ts
import * as test from "edge://assert/mod.ts";
```

Or selective imports:

```ts
import { assertEquals, assertThrows, runSuite } from "edge://assert/mod.ts";
```

## Error Type

### `AssertionError`

```ts
class AssertionError extends Error
```

Thrown by all assertion helpers on failure.

## Core Assertions

### `assert(condition, message?)`

```ts
function assert(condition: unknown, message?: string): asserts condition
```

Fails when `condition` is falsy.

### `assertEquals(actual, expected, message?)`

```ts
function assertEquals<T>(actual: T, expected: T, message?: string): void
```

Deep equality assertion. Supports:

- primitives
- arrays
- typed arrays (`ArrayBuffer` views)
- plain objects
- `Date`
- `RegExp`
- `Set`
- `Map`

Failure message includes expected and actual values.

### `assertNotEquals(actual, expected, message?)`

```ts
function assertNotEquals<T>(actual: T, expected: T, message?: string): void
```

Fails when values are deeply equal.

### `assertStrictEquals(actual, expected, message?)`

```ts
function assertStrictEquals<T>(actual: T, expected: T, message?: string): void
```

Uses `Object.is(actual, expected)`.

### `assertExists(value, message?)`

```ts
function assertExists<T>(value: T, message?: string): asserts value is NonNullable<T>
```

Fails when value is `null` or `undefined`.

## Pattern and Collection Assertions

### `assertMatch(text, regex, message?)`

```ts
function assertMatch(text: string, regex: RegExp, message?: string): void
```

Fails when `regex.test(text)` is false.

### `assertArrayIncludes(array, values, message?)`

```ts
function assertArrayIncludes<T>(
  array: readonly T[],
  values: readonly T[],
  message?: string,
): void
```

Fails if any item in `values` is missing from `array`.

### `assertObjectMatch(actual, expected, message?)`

```ts
function assertObjectMatch(
  actual: Record<string, unknown>,
  expected: Record<string, unknown>,
  message?: string,
): void
```

Subset assertion. Fails if any key in `expected` is missing in `actual` or if a value differs.

## Exception Assertions

### `assertThrows(fn, ErrorClassOrMessage?, message?)`

```ts
function assertThrows(
  fn: () => unknown,
  ErrorClassOrMessage?: (new (...args: any[]) => Error) | string,
  message?: string,
): Error
```

Behavior:

- Fails if `fn` does not throw
- Optionally validates thrown error type when an `ErrorClass` is provided
- Returns the thrown `Error`

Examples:

```ts
assertThrows(() => {
  throw new Error("boom");
});

assertThrows(
  () => {
    throw new TypeError("bad");
  },
  TypeError,
);
```

### `assertRejects(fn, ErrorClassOrMessage?, message?)`

```ts
function assertRejects(
  fn: () => Promise<unknown>,
  ErrorClassOrMessage?: (new (...args: any[]) => Error) | string,
  message?: string,
): Promise<Error>
```

Behavior:

- Fails if the promise resolves
- Optionally validates rejection error type
- Resolves with the rejected `Error`

## Minimal Test Runner

### `TestCase`

```ts
type TestCase = {
  name: string;
  run: () => void | Promise<void>;
  ignore?: boolean;
  only?: boolean;
};
```

### `TestSuite`

```ts
type TestSuite = {
  name: string;
  tests: TestCase[];
  ignore?: boolean;
  only?: boolean;
};
```

### Test Helpers

```ts
function test(name: string, run: () => void | Promise<void>): TestCase
function testIgnore(name: string, run: () => void | Promise<void>): TestCase
function testOnly(name: string, run: () => void | Promise<void>): TestCase

function suite(name: string, tests: TestCase[]): TestSuite
function suiteIgnore(name: string, tests: TestCase[]): TestSuite
function suiteOnly(name: string, tests: TestCase[]): TestSuite
```

Use these helpers when you want clear `ignore`/`only` semantics without manually setting flags.

### `runSuite(suiteName, tests)`

```ts
function runSuite(
  suiteName: string,
  tests: TestCase[],
  options?: { ignore?: boolean; only?: boolean },
): Promise<void>
```

Runs tests sequentially and logs:

- suite start
- per-test success line
- skipped tests (`skip - <name>`) when `ignore` is set
- suite summary

Behavior:

- If any test has `only: true`, only those tests run.
- Tests with `ignore: true` are skipped.
- If `options.ignore` is true, the whole suite is ignored.

### `runSuites(suites)`

```ts
function runSuites(suites: TestSuite[]): Promise<void>
```

Runs multiple suites in order.

Behavior:

- If any suite has `only: true`, only those suites run.
- Suites with `ignore: true` are skipped.

Example:

```ts
import {
  runSuite,
  runSuites,
  suite,
  suiteIgnore,
  test,
  testIgnore,
  testOnly,
  assertEquals,
} from "edge://assert/mod.ts";

await runSuite("example", [
  test("works", () => assertEquals(2 * 3, 6)),
  testIgnore("skip me", () => assertEquals(1, 2)),
]);

await runSuites([
  suite("math", [
    testOnly("multiply", () => assertEquals(3 * 3, 9)),
  ]),
  suiteIgnore("integration", [
    test("expensive", () => assertEquals(1, 1)),
  ]),
]);
```
