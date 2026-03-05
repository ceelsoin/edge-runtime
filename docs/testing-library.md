# Testing Library for Edge Runtime

This project ships a built-in assertion and test helper library for JavaScript/TypeScript tests running inside the edge runtime.

## Overview

The test library is designed for edge environments:

- No Node.js built-ins required
- No external dependencies
- TypeScript-friendly API
- Works with the runtime test command

Import path for user tests:

```ts
import {
  runSuite,
  runSuites,
  test,
  testIgnore,
  testOnly,
  suite,
  suiteIgnore,
  suiteOnly,
  assert,
  assertEquals,
  assertRejects,
} from "edge://assert/mod.ts";
```

## How to Run Tests

Use the CLI test command with a path or glob pattern:

```bash
cargo run -- test --path "./tests/js/**/*.ts"
```

Optional ignore patterns:

```bash
cargo run -- test --path "./tests/js/**/*.ts" --ignore "./tests/js/helpers/**"
```

Makefile shortcut used by this repository:

```bash
make test-js
```

## Minimal Example

```ts
import { runSuite, test, testIgnore, assertEquals } from "edge://assert/mod.ts";

await runSuite("math", [
  test("sum", () => {
    assertEquals(1 + 1, 2);
  }),
  testIgnore("future case", () => {
    assertEquals(1 + 2, 4);
  }),
]);
```

## Ignore and Only Controls

You can control execution in two levels:

- Test level: `ignore` / `only`
- Suite level: `ignore` / `only`

Helpers:

- `testIgnore(...)`
- `testOnly(...)`
- `suiteIgnore(...)`
- `suiteOnly(...)`

Rules:

- If any test in a suite has `only: true`, only those tests execute.
- If any suite has `only: true`, only those suites execute via `runSuites`.
- `ignore: true` skips test or suite.

Example with multiple suites:

```ts
import {
  runSuites,
  suite,
  suiteOnly,
  test,
  testOnly,
  assertEquals,
} from "edge://assert/mod.ts";

await runSuites([
  suiteOnly("core", [
    testOnly("strict", () => assertEquals(2, 2)),
    test("normal", () => assertEquals(3, 3)),
  ]),
  suite("integration", [
    test("http", () => assertEquals(200, 200)),
  ]),
]);
```

## Design Notes

- `runSuite` is intentionally lightweight and only prints simple progress lines.
- Assertions throw `AssertionError` on failure.
- Deep equality is supported in `assertEquals` for common edge-runtime types.
- The library is dependency-free to keep startup and bundle costs low.

## Assertion Coverage

Current assertion helpers include:

- `assert`
- `assertEquals`
- `assertNotEquals`
- `assertStrictEquals`
- `assertExists`
- `assertMatch`
- `assertArrayIncludes`
- `assertObjectMatch`
- `assertThrows` (with optional error type validation)
- `assertRejects` (with optional error type validation)

Current runner helpers include:

- `runSuite`
- `runSuites`
- `test`, `testIgnore`, `testOnly`
- `suite`, `suiteIgnore`, `suiteOnly`

For signatures and behavior details, see `docs/testing-api-reference.md`.
