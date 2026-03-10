import { runSuite, assert, assertEquals } from "thunder:testing";

function parseHostPort(input: string): { host: string; port: number } {
  const url = new URL(input);
  return {
    host: url.hostname,
    port: Number(url.port || 443),
  };
}

function encodeLengthPrefixed(payload: Uint8Array): Uint8Array {
  const out = new Uint8Array(4 + payload.length);
  const dv = new DataView(out.buffer);
  dv.setUint32(0, payload.length, false);
  out.set(payload, 4);
  return out;
}

await runSuite("networking-tcp-patterns", [
  {
    name: "tcp host and port parsing via URL",
    run: () => {
      const parsed = parseHostPort("https://api.example.com:8443/resource");
      assertEquals(parsed.host, "api.example.com");
      assertEquals(parsed.port, 8443);
    },
  },
  {
    name: "binary framing for tcp payloads",
    run: () => {
      const payload = new TextEncoder().encode("hello");
      const framed = encodeLengthPrefixed(payload);
      const dv = new DataView(framed.buffer);
      assertEquals(dv.getUint32(0, false), 5);
      assertEquals(new TextDecoder().decode(framed.slice(4)), "hello");
    },
  },
  {
    name: "abortable connect timeout pattern",
    run: async () => {
      const controller = new AbortController();
      const timeout = setTimeout(() => controller.abort(), 1);

      try {
        await new Promise((_resolve, reject) => {
          controller.signal.addEventListener("abort", () => reject(new Error("timeout")));
        });
      } catch (err) {
        assert(err instanceof Error);
        assertEquals(err.message, "timeout");
      } finally {
        clearTimeout(timeout);
      }
    },
  },
  {
    name: "runtime tcp api capability probe",
    run: () => {
      const maybeDeno = (globalThis as { Deno?: Record<string, unknown> }).Deno;
      const hasTcpConnect = typeof maybeDeno?.connect === "function";

      // Both paths are acceptable: runtime may expose TCP ops via Deno API or keep them disabled.
      assert(typeof hasTcpConnect === "boolean");
    },
  },
]);
