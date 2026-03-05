# Testing Library for Edge Runtime

This testing library is built for edge/serverless environments (Supabase Edge Runtime, Deno-like runtimes, Cloudflare Workers, browser-compatible runtimes).

Design constraints:

- no Node.js built-ins
- no external dependencies
- lightweight runtime footprint
- TypeScript-friendly API

## 1. Assertions

Core assertions:

- `assert(condition, message?)`
- `assertEquals(actual, expected, message?)`
- `assertNotEquals(actual, expected, message?)`
- `assertStrictEquals(actual, expected, message?)`
- `assertNotStrictEquals(actual, expected, message?)`
- `assertExists(value, message?)`
- `assertInstanceOf(value, Type, message?)`
- `assertMatch(text, regex, message?)`
- `assertArrayIncludes(array, values, message?)`
- `assertObjectMatch(actual, expected, message?)`
- `assertThrows(fn, ErrorClassOrMessage?, message?)`
- `assertRejects(fn, ErrorClassOrMessage?, message?)`
- `assertType<T>(value)` (compile-time helper)

```ts
import {
  assert,
  assertEquals,
  assertInstanceOf,
  assertThrows,
  assertType,
} from "edge://assert/mod.ts";

const value: unknown = new Error("boom");

assert(true);
assertEquals(1 + 1, 2);
assertInstanceOf(value, Error);
assertThrows(() => {
  throw new Error("fail");
});
assertType<number>(123);
```

`assertEquals` includes a readable diff output in failures.

## 2. Writing Tests

Use `test(...)` for individual cases and `runSuite(...)` / `runSuites(...)` for execution.

```ts
import { runSuite, test, assertEquals } from "edge://assert/mod.ts";

await runSuite("math", [
  test("sum works", () => {
    assertEquals(1 + 1, 2);
  }),
]);
```

## 3. Test Suites

Suite helpers:

- `suite(name, entries)`
- `suiteIgnore(name, entries)`
- `suiteOnly(name, entries)`
- `runSuite(name, entries)`
- `runSuites([suite(...)])`

Test helpers:

- `test(name, fn, options?)`
- `testIgnore(name, fn, options?)`
- `testOnly(name, fn, options?)`

Options:

- `ignore?: boolean`
- `only?: boolean`
- `timeout?: number`
- `concurrent?: boolean`
- `retry?: number`

## 4. Lifecycle Hooks

Hook helpers:

- `beforeAll(fn)`
- `afterAll(fn)`
- `beforeEach(fn)`
- `afterEach(fn)`

```ts
import {
  runSuite,
  beforeAll,
  beforeEach,
  afterEach,
  afterAll,
  test,
  assert,
} from "edge://assert/mod.ts";

await runSuite("users", [
  beforeAll(async () => {
    await Promise.resolve();
  }),

  beforeEach(() => {
    // reset state
  }),

  test("create user", () => {
    assert(true);
  }),

  test("delete user", () => {
    assert(true);
  }),

  afterEach(() => {
    // cleanup
  }),

  afterAll(() => {
    // shutdown
  }),
]);
```

## 5. Mocking

### mockFn

```ts
import { mockFn, assertEquals } from "edge://assert/mod.ts";

const add = mockFn((a: number, b: number) => a + b);
add(1, 2);

assertEquals(add.calls.length, 1);
assertEquals(add.calls[0].args, [1, 2]);
assertEquals(add.calls[0].result, 3);

add.mockClear();
```

### spyOn

```ts
import { spyOn, assertEquals } from "edge://assert/mod.ts";

const spy = spyOn(console, "log");
console.log("hello");

assertEquals(spy.calls.length, 1);
assertEquals(spy.calls[0].args, ["hello"]);

spy.restore();
```

### Mock assertions

- `assertSpyCalls(mockOrSpy, count)`
- `assertSpyCall(mockOrSpy, index, { args?, result?, error? })`

```ts
import {
  mockFn,
  assertSpyCalls,
  assertSpyCall,
} from "edge://assert/mod.ts";

const fn = mockFn((a: number, b: number) => a + b);
fn(1, 2);

assertSpyCalls(fn, 1);
assertSpyCall(fn, 0, { args: [1, 2], result: 3 });
```

## 6. Snapshot Testing

Use `assertSnapshot(value, options?)`.

Default behavior:

- stores snapshots in `__snapshots__/`
- file name: `<test-file>.snap`
- key: current test name
- format: JSON

```ts
import { assertSnapshot } from "edge://assert/mod.ts";

const user = { id: 1, name: "Celso" };
assertSnapshot(user);
```

Optional config:

- `name?: string`
- `filePath?: string`
- `update?: boolean`

```ts
assertSnapshot(user, { name: "user-v1" });
```

## 7. Fake Timers

Use `mockTime()` for deterministic timer control.

```ts
import { mockTime, assert } from "edge://assert/mod.ts";

const clock = mockTime();

try {
  let called = false;
  setTimeout(() => {
    called = true;
  }, 1000);

  clock.tick(1000);
  assert(called);
} finally {
  clock.restore();
}
```

## 8. HTTP Mocking

### mockFetch (route map)

```ts
import { mockFetch, assertEquals } from "edge://assert/mod.ts";

const mock = mockFetch({
  "https://api.test/users": {
    status: 200,
    body: { users: [] },
  },
});

try {
  const res = await fetch("https://api.test/users");
  assertEquals(await res.json(), { users: [] });
} finally {
  mock.restore();
}
```

### mockFetchHandler (dynamic)

```ts
import { mockFetchHandler, assertEquals } from "edge://assert/mod.ts";

const mock = mockFetchHandler((req) => {
  if (req.url.endsWith("/users")) {
    return new Response(JSON.stringify({ users: [] }), {
      status: 200,
      headers: { "content-type": "application/json" },
    });
  }
  return new Response("not found", { status: 404 });
});

try {
  const res = await fetch("https://api.test/users");
  assertEquals(res.status, 200);
} finally {
  mock.restore();
}
```

## 9. Concurrent Tests

Set `{ concurrent: true }` in test options.

```ts
import { runSuite, test } from "edge://assert/mod.ts";

await runSuite("concurrent", [
  test("a", async () => {
    await new Promise((r) => setTimeout(r, 30));
  }, { concurrent: true }),
  test("b", async () => {
    await new Promise((r) => setTimeout(r, 30));
  }, { concurrent: true }),
]);
```

Concurrent tests run via `Promise.all`.

## 10. Table-driven Tests

Use `testEach(rows)`.

```ts
import { runSuite, testEach, assertEquals } from "edge://assert/mod.ts";

await runSuite("sum", [
  ...testEach([
    [1, 2, 3] as const,
    [2, 3, 5] as const,
  ])("sum test", (a, b, result) => {
    assertEquals(a + b, result);
  }),
]);
```

## 11. Conditional Tests

Use `testIf(condition)`.

```ts
import { runSuite, testIf, assert } from "edge://assert/mod.ts";

const featureEnabled = typeof Deno === "object";

await runSuite("feature-gated", [
  testIf(featureEnabled)("feature test", () => {
    assert(true);
  }),
]);
```

If the condition is false, the test is skipped.

## Timeouts and Retries

`timeout` fails tests that run too long.

`retry` retries flaky tests before final failure.

```ts
import { runSuite, test } from "edge://assert/mod.ts";

let attempt = 0;

await runSuite("resilience", [
  test("slow request", async () => {
    await new Promise((r) => setTimeout(r, 10));
  }, { timeout: 1000 }),

  test("flaky test", () => {
    attempt += 1;
    if (attempt < 3) {
      throw new Error("flaky");
    }
  }, { retry: 3 }),
]);
```

## Running Tests in This Repository

```bash
cargo run -- test --path "./tests/js/**/*.ts" --ignore "./tests/js/lib/**"
```

```bash
make test-js
```
