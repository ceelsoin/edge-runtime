import { runSuite, assert, assertEquals, assertExists, assertRejects } from "thunder:testing";

await runSuite("crypto-and-timers", [
  {
    name: "crypto primitives",
    run: () => {
      const bytes = new Uint8Array(32);
      crypto.getRandomValues(bytes);
      assert(bytes.some((v) => v !== 0), "random bytes should not be all zero");

      const id = crypto.randomUUID();
      assertEquals(typeof id, "string");
      assert(id.length >= 32, "uuid should look valid");
      assertExists(crypto.subtle);
    },
  },
  {
    name: "subtle.digest sha-256",
    run: async () => {
      const data = new TextEncoder().encode("thunder");
      const digest = await crypto.subtle.digest("SHA-256", data);
      assertEquals(digest.byteLength, 32);
    },
  },
  {
    name: "timers",
    run: async () => {
      const before = performance.now();
      await new Promise<void>((resolve) => {
        const id = setTimeout(() => {
          clearTimeout(id);
          resolve();
        }, 5);
      });
      const elapsed = performance.now() - before;
      assert(elapsed >= 0, "performance should move forward");

      let ticks = 0;
      await new Promise<void>((resolve) => {
        const id = setInterval(() => {
          ticks += 1;
          if (ticks >= 2) {
            clearInterval(id);
            resolve();
          }
        }, 1);
      });
      assert(ticks >= 2, "interval should tick");
    },
  },
  {
    name: "abort signal cancels async wait",
    run: async () => {
      const controller = new AbortController();
      const p = new Promise((_resolve, reject) => {
        controller.signal.addEventListener("abort", () => reject(new Error("aborted")));
      });
      controller.abort();
      await assertRejects(() => p as Promise<unknown>);
    },
  },
]);
