import { runSuite, assert, assertEquals, assertThrows } from "thunder:testing";

await runSuite("language-engine-compat", [
  {
    name: "optional chaining and nullish coalescing",
    run: () => {
      const data: { nested?: { value?: number } } = {};
      const value = data.nested?.value ?? 42;
      assertEquals(value, 42);
    },
  },
  {
    name: "class private fields",
    run: () => {
      class Counter {
        #value = 0;
        inc() {
          this.#value += 1;
        }
        get value() {
          return this.#value;
        }
      }

      const c = new Counter();
      c.inc();
      assertEquals(c.value, 1);
    },
  },
  {
    name: "async generator and for-await",
    run: async () => {
      async function* seq() {
        yield 1;
        yield 2;
        yield 3;
      }

      const out: number[] = [];
      for await (const n of seq()) {
        out.push(n);
      }
      assertEquals(out.join(","), "1,2,3");
    },
  },
  {
    name: "promise combinators",
    run: async () => {
      const settled = await Promise.allSettled([
        Promise.resolve("ok"),
        Promise.reject(new Error("no")),
      ]);
      assertEquals(settled.length, 2);
      assert(settled.some((x) => x.status === "fulfilled"));
      assert(settled.some((x) => x.status === "rejected"));
    },
  },
  {
    name: "proxy and reflect",
    run: () => {
      const target: Record<string, unknown> = {};
      const p = new Proxy(target, {
        set(obj, prop, value) {
          return Reflect.set(obj, prop, value);
        },
      });
      p.answer = 42;
      assertEquals(Reflect.get(target, "answer"), 42);
    },
  },
  {
    name: "intl and error types",
    run: () => {
      const n = new Intl.NumberFormat("en-US").format(1000);
      assert(n.includes("1"));
      assertThrows(() => {
        throw new TypeError("boom");
      });
    },
  },
]);
