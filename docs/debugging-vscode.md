# Debugging Edge Functions with VS Code

This guide explains how to use the V8 inspector protocol to debug TypeScript edge functions directly in VS Code, with working breakpoints, variable inspection, and source map navigation.

---

## Prerequisites

- VS Code with the **JavaScript Debugger** extension (built-in since VS Code 1.60)
- A debug build of the runtime: `cargo build`
- A `.vscode/launch.json` in the project root (already provided)

---

## Debugging Modes

### 1. `watch --inspect` — Attach and continue

Starts the inspector server and immediately continues running. VS Code attaches after the process is already listening for requests. Breakpoints set in TypeScript files will be hit the next time a matching request arrives.

```bash
cargo run -- watch --path ./examples/hello/hello.ts --inspect
```

Port defaults to `9229`. To use a different port:

```bash
cargo run -- watch --path ./examples/hello/hello.ts --inspect 9230
```

Use the **"Attach Edge Runtime (hello:9229)"** launch configuration, which has `continueOnAttach: true` so VS Code does not pause on attach.

---

### 2. `watch --inspect-brk` — Break on first statement

Starts the inspector server and **pauses execution before the first line of user code** runs. The process blocks until VS Code attaches. Useful for debugging module initialization or the very first request handler invocation.

```bash
cargo run -- watch --path ./examples/hello/hello.ts --inspect --inspect-brk
```

> Note: `--inspect` is required alongside `--inspect-brk`. The `--inspect-brk` flag only sets the break-on-first-statement flag; `--inspect` enables the server.

Use the **"Attach Edge Runtime (hello:9229, inspect-brk)"** launch configuration. Do **not** add `continueOnAttach: true` here — you want VS Code to pause at the first statement.

---

### 3. `test --inspect` — Debug a single test file

Attaches the inspector to a test run. Requires exactly one test file (multiple files are not supported when `--inspect` is active, since each function would need its own port).

```bash
cargo run -- test --path ./tests/js/my-test.ts --inspect
```

Use the **"Attach Edge Runtime (hello:9229)"** configuration (or add a dedicated one for your test file name).

---

## VS Code Launch Configurations

The provided [.vscode/launch.json](../.vscode/launch.json) contains three ready-to-use configurations:

| Configuration | Mode | `continueOnAttach` | Use case |
|---|---|---|---|
| `Attach Edge Runtime (hello:9229)` | `--inspect` | `true` | Normal debugging; break only on explicit breakpoints |
| `Attach Edge Runtime (hello:9229, inspect-brk)` | `--inspect-brk` | _(not set)_ | Break at the very first statement |
| `Attach Edge Runtime (hello:9229, trace)` | `--inspect` | `true` | Verbose CDP trace log for troubleshooting the debugger itself |

All configurations use:
- `"type": "node"` — required to connect to Deno's V8 inspector (which advertises itself as a Node target)
- `"request": "attach"` — connects to an already-running process
- `"sourceMaps": true` — enables automatic TypeScript source map resolution

---

## Step-by-Step: Debugging a Function

1. Start the watch server with `--inspect`:

   ```bash
   cargo run -- watch --path ./examples/hello/hello.ts --inspect
   ```

   You should see output like:
   ```
   Inspector server started on 127.0.0.1:9229 (target: hello)
   Watching for file changes...
   ```

2. Open VS Code, go to the **Run and Debug** panel (`Ctrl+Shift+D` / `Cmd+Shift+D`).

3. Select **"Attach Edge Runtime (hello:9229)"** and click the green play button (or press `F5`).

4. Set a breakpoint in your TypeScript source file (e.g., `examples/hello/hello.ts`).

5. Send a request to the function:

   ```bash
   curl http://localhost:9000/hello
   ```

6. VS Code stops at your breakpoint. You can inspect variables, step through code, and use the debug console.

---

## Debugging Multiple Functions

When multiple functions are loaded, each gets its own port assigned sequentially starting from the base port:

```bash
cargo run -- watch --path ./examples --inspect 9229
# hello → port 9229
# json-api → port 9230
# cors → port 9231
# ...
```

Add a launch configuration for each function you want to debug:

```json
{
    "name": "Attach Edge Runtime (json-api:9230)",
    "type": "node",
    "request": "attach",
    "port": 9230,
    "continueOnAttach": true,
    "sourceMaps": true
}
```

---

## Using `debugger` Statements

You can add `debugger;` anywhere in your TypeScript source to force a breakpoint, even without setting one in VS Code:

```typescript
export default async function handler(req: Request): Promise<Response> {
    debugger; // VS Code will pause here when attached
    const body = await req.json();
    return Response.json({ received: body });
}
```

When using `--inspect` (not `--inspect-brk`), `debugger;` statements are only hit after VS Code is already attached. With `--inspect-brk`, the process waits for attachment before executing anything, so all `debugger;` statements will be reachable.

---

## How Source Maps Work

TypeScript files are compiled and bundled into `.eszip` archives during the watch build step. The compiler emits a separate source map for each module, which is stored alongside the compiled output inside the eszip bundle.

When a module is loaded into V8, the runtime attaches the source map as an inline base64 data URL:

```
//# sourceMappingURL=data:application/json;base64,<encoded-map>
```

V8 reads this annotation and notifies the debugger via the `Debugger.scriptParsed` CDP event with the `sourceMapURL` field populated. VS Code resolves the original TypeScript file paths from the `sources` array inside the map and opens them automatically when a breakpoint is hit.

No manual source map path configuration is required.

---

## Troubleshooting

### "Cannot connect to target" or "Connection refused"

- Ensure the runtime is running before clicking attach in VS Code.
- Verify the port matches between the `--inspect` argument and the launch configuration.
- Check that no other process is using the port: `lsof -i :9229`.

### "Unknown Source" frames in the call stack

This happens when VS Code receives a `Debugger.paused` event before it has processed `Debugger.scriptParsed` for the relevant script. It means the debugger attached but the event loop was not flushed in time.

Causes and fixes:
- Make sure you are on the latest build: `cargo build`.
- Try `--inspect-brk` instead of `--inspect` — it forces an event loop flush before any code runs.
- If the issue persists, use the trace configuration (`Attach Edge Runtime (hello:9229, trace)`) to capture the raw CDP message log and verify `scriptParsed` precedes `paused`.

### Breakpoints not hit

- Ensure `sourceMaps: true` is set in the launch configuration.
- Ensure the breakpoint is in a TypeScript file that is actually part of the loaded function (not a dependency that isn't compiled into the bundle).
- After attaching, trigger the function with a real HTTP request — the runtime only executes handler code when a request arrives.

### Port already in use after a crash

The inspector TCP port is held until the process fully exits. If the process crashed and the port is stuck:

```bash
lsof -ti :9229 | xargs kill -9
```

---

## Technical Notes

- The V8 inspector uses the **Chrome DevTools Protocol (CDP)** over WebSocket.
- The runtime serves a CDP-compatible `/json/list` endpoint on the same port. VS Code reads this endpoint to discover the debug target before opening the WebSocket connection.
- Each debug session gets a unique UUID target ID, which becomes the WebSocket path (`ws://127.0.0.1:<port>/<uuid>`). This is required by the vscode-js-debug adapter.
- The `"type": "node"` adapter is used (not `"chrome"`) because the runtime advertises `"type": "node"` targets in `/json/list`, which is consistent with how Deno and Node.js expose their V8 inspector.
- The `/json/version` endpoint returns `"Browser": "node.js/v18.0.0"`, which is the version string expected by the vscode-js-debug protocol handshake.
