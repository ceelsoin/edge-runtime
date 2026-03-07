import { runSuite, assert, assertEquals } from "edge://assert/mod.ts";

await runSuite("js-builtins-and-collections", [
  {
    name: "collections map/set",
    run: () => {
      const map = new Map<string, number>();
      map.set("a", 1);
      const set = new Set<number>([1, 2, 2, 3]);

      assertEquals(map.get("a"), 1);
      assertEquals(set.size, 3);
    },
  },
  {
    name: "collections weakmap/weakset",
    run: () => {
      const key = {};
      const wm = new WeakMap<object, number>();
      const ws = new WeakSet<object>();

      wm.set(key, 10);
      ws.add(key);

      assertEquals(wm.get(key), 10);
      assert(ws.has(key));
    },
  },
  {
    name: "symbol and reflect",
    run: () => {
      const sym = Symbol("token");
      const obj: Record<PropertyKey, unknown> = {};

      Reflect.set(obj, sym, 123);
      assertEquals(Reflect.get(obj, sym), 123);
      assertEquals(typeof sym, "symbol");
    },
  },
  {
    name: "json and promise constructor",
    run: async () => {
      const json = JSON.stringify({ ok: true, n: 1 });
      const parsed = JSON.parse(json) as { ok: boolean; n: number };
      assert(parsed.ok);
      assertEquals(parsed.n, 1);

      const value = await new Promise<number>((resolve) => resolve(7));
      assertEquals(value, 7);
    },
  },
  {
    name: "string array object methods",
    run: () => {
      const str = "thunder".toUpperCase();
      const arr = [1, 2, 3].map((x) => x * 2).filter((x) => x > 2);
      const obj = { a: 1, b: 2 };

      assertEquals(str, "THUNDER");
      assertEquals(arr.join(","), "4,6");
      assertEquals(Object.keys(obj).length, 2);
      assertEquals(Object.entries(obj)[0][0], "a");
    },
  },
  {
    name: "math date regexp",
    run: () => {
      const rounded = Math.round(2.6);
      const date = new Date("2025-01-01T00:00:00.000Z");
      const re = /thunder/;

      assertEquals(rounded, 3);
      assertEquals(date.toISOString(), "2025-01-01T00:00:00.000Z");
      assert(re.test("thunder"));
    },
  },
]);
