import { runSuite, assert, assertEquals, assertExists } from "edge://assert/mod.ts";

await runSuite("web-apis-full", [
  {
    name: "fetch primitives are available",
    run: () => {
      assert(typeof fetch === "function", "fetch should exist");
      assert(typeof Headers === "function", "Headers should exist");
      assert(typeof Request === "function", "Request should exist");
      assert(typeof Response === "function", "Response should exist");
      assert(typeof FormData === "function", "FormData should exist");
    },
  },
  {
    name: "url and url pattern",
    run: () => {
      const url = new URL("https://example.com/path?q=1");
      assertEquals(url.pathname, "/path");
      assertEquals(url.searchParams.get("q"), "1");
      assert(typeof URLPattern === "function", "URLPattern should exist");
      assert(typeof URL.parse === "function", "URL.parse should exist");
    },
  },
  {
    name: "streams and queueing strategies",
    run: () => {
      assert(typeof ReadableStream === "function");
      assert(typeof WritableStream === "function");
      assert(typeof TransformStream === "function");
      assert(typeof ByteLengthQueuingStrategy === "function");
      assert(typeof CountQueuingStrategy === "function");
    },
  },
  {
    name: "encoding and compression",
    run: () => {
      const bytes = new TextEncoder().encode("abc");
      assertEquals(bytes.length, 3);
      const decoded = new TextDecoder().decode(new Uint8Array([72, 105]));
      assertEquals(decoded, "Hi");
      assertEquals(atob(btoa("hello")), "hello");
      assert(typeof CompressionStream === "function");
      assert(typeof DecompressionStream === "function");
      assert(typeof TextEncoderStream === "function");
      assert(typeof TextDecoderStream === "function");
    },
  },
  {
    name: "events and dom-like objects",
    run: () => {
      const et = new EventTarget();
      let called = false;
      et.addEventListener("ping", () => {
        called = true;
      });
      et.dispatchEvent(new Event("ping"));
      assert(called, "EventTarget should dispatch");

      const cloned = structuredClone({ a: 1, b: [1, 2] });
      assertEquals(cloned.a, 1);
      assertEquals(cloned.b.length, 2);

      const blob = new Blob(["abc"]);
      assertEquals(blob.size, 3);

      const file = new File(["hello"], "hello.txt", { type: "text/plain" });
      assertEquals(file.name, "hello.txt");
      assertExists(file.type);
    },
  },
]);
