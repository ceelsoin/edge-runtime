import {
  runSuite,
  assert,
  assertEquals,
  mockFn,
  spyOn,
  mockFetch,
  mockFetchHandler,
  mockTime,
  assertSpyCalls,
  assertSpyCall,
} from "edge://assert/mod.ts";

await runSuite("mocking-system", [
  {
    name: "mockFn tracks calls and results",
    run: () => {
      const add = mockFn((a: number, b: number) => a + b);
      const value = add(2, 3);

      assertEquals(value, 5);
      assertSpyCalls(add, 1);
      assertSpyCall(add, 0, { args: [2, 3], result: 5 });

      add.mockClear();
      assertEquals(add.calls.length, 0);
    },
  },
  {
    name: "spyOn captures calls and restores original",
    run: () => {
      const obj = {
        value: 1,
        inc(n: number) {
          this.value += n;
          return this.value;
        },
      };

      const spy = spyOn(obj, "inc");
      const out = obj.inc(2);

      assertEquals(out, 3);
      assertSpyCalls(spy, 1);
      assertSpyCall(spy, 0, { args: [2], result: 3 });

      spy.restore();
      const out2 = obj.inc(1);
      assertEquals(out2, 4);
    },
  },
  {
    name: "mockFetch returns static route responses",
    run: async () => {
      const mock = mockFetch({
        "https://api.test/users": {
          status: 200,
          body: { name: "Celso" },
          headers: { "x-mock": "yes" },
        },
      });

      try {
        const res = await fetch("https://api.test/users");
        assertEquals(res.status, 200);
        assertEquals(res.headers.get("x-mock"), "yes");
        assertEquals(await res.json(), { name: "Celso" });
      } finally {
        mock.restore();
      }
    },
  },
  {
    name: "mockFetchHandler handles dynamic requests",
    run: async () => {
      const mock = mockFetchHandler((req: Request) => {
        if (req.url.endsWith("/users")) {
          return new Response(JSON.stringify({ users: [] }), {
            status: 200,
            headers: { "content-type": "application/json" },
          });
        }
        return new Response("not found", { status: 404 });
      });

      try {
        const ok = await fetch("https://api.test/users");
        assertEquals(ok.status, 200);
        assertEquals(await ok.json(), { users: [] });

        const notFound = await fetch("https://api.test/other");
        assertEquals(notFound.status, 404);
      } finally {
        mock.restore();
      }
    },
  },
  {
    name: "mockTime advances timeout callbacks",
    run: () => {
      const clock = mockTime();

      try {
        let called = false;
        setTimeout(() => {
          called = true;
        }, 1000);

        clock.tick(999);
        assert(!called);

        clock.tick(1);
        assert(called);
      } finally {
        clock.restore();
      }
    },
  },
]);
