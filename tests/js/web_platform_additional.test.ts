import { runSuite, assert, assertEquals } from "thunder:testing";

await runSuite("web-platform-additional", [
  {
    name: "messaging apis",
    run: () => {
      const channel = new MessageChannel();

      // Validate API presence and object shape.
      assert(channel.port1 instanceof MessagePort);
      assert(channel.port2 instanceof MessagePort);
      assert(typeof channel.port1.postMessage === "function");
      assert(typeof channel.port1.close === "function");

      // Close ports explicitly to avoid leaving pending resources in the runtime.
      channel.port1.close();
      channel.port2.close();
    },
  },
  {
    name: "image data api",
    run: () => {
      const img = new ImageData(2, 2);
      assertEquals(img.width, 2);
      assertEquals(img.height, 2);
      assertEquals(img.data.length, 16);
    },
  },
  {
    name: "performance mark and measure",
    run: () => {
      performance.mark("compat-start");
      performance.mark("compat-end");
      performance.measure("compat-measure", "compat-start", "compat-end");

      const marks = performance.getEntriesByType("mark");
      const measures = performance.getEntriesByType("measure");

      assert(marks.length >= 2, "expected at least 2 marks");
      assert(measures.length >= 1, "expected at least 1 measure");

      performance.clearMarks("compat-start");
      performance.clearMarks("compat-end");
      performance.clearMeasures("compat-measure");
    },
  },
  {
    name: "console extended methods",
    run: () => {
      assert(typeof console.table === "function");
      assert(typeof console.trace === "function");
      assert(typeof console.dir === "function");
    },
  },
  {
    name: "typed arrays and data view",
    run: () => {
      const buffer = new ArrayBuffer(16);
      const view = new DataView(buffer);
      view.setInt32(0, 42, true);
      const int32 = new Int32Array(buffer, 0, 1);
      const float64 = new Float64Array(buffer, 8, 1);
      float64[0] = Math.PI;

      assertEquals(view.getInt32(0, true), 42);
      assertEquals(int32[0], 42);
      assert(float64[0] > 3.14 && float64[0] < 3.15);
    },
  },
]);
