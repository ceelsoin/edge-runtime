import type { MockCall } from "./mockFn.ts";

export type MockFetchResponse = {
  status?: number;
  body?: unknown;
  headers?: Record<string, string>;
};

export type MockFetchRoutes = Record<string, MockFetchResponse>;

export type MockFetchController = {
  calls: MockCall[];
  restore: () => void;
};

export type MockFetchHandler = (request: Request) => Response | Promise<Response> | null | undefined;

function isBodyInitLike(value: unknown): value is BodyInit {
  return typeof value === "string"
    || value instanceof Blob
    || value instanceof FormData
    || value instanceof URLSearchParams
    || value instanceof ReadableStream
    || value instanceof ArrayBuffer
    || ArrayBuffer.isView(value);
}

function buildResponse(spec: MockFetchResponse): Response {
  const status = spec.status ?? 200;
  const headers = new Headers(spec.headers);

  if (spec.body === undefined) {
    return new Response(null, { status, headers });
  }

  if (isBodyInitLike(spec.body)) {
    return new Response(spec.body, { status, headers });
  }

  if (!headers.has("content-type")) {
    headers.set("content-type", "application/json");
  }

  return new Response(JSON.stringify(spec.body), { status, headers });
}

function withMockedFetch(
  handler: (request: Request) => Promise<Response>,
): MockFetchController {
  const originalFetch = globalThis.fetch;
  if (typeof originalFetch !== "function") {
    throw new Error("globalThis.fetch is not available in this runtime");
  }

  const calls: MockCall[] = [];
  let restored = false;

  globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
    const request = new Request(input, init);
    const call: MockCall = { args: [request] };
    calls.push(call);

    try {
      const response = await handler(request);
      call.result = response;
      return response;
    } catch (error) {
      call.error = error;
      throw error;
    }
  }) as typeof fetch;

  return {
    calls,
    restore: () => {
      if (restored) return;
      restored = true;
      globalThis.fetch = originalFetch;
    },
  };
}

export function mockFetch(routes: MockFetchRoutes): MockFetchController {
  return withMockedFetch(async (request) => {
    const route = routes[request.url];
    if (!route) {
      return new Response(`No mocked response for ${request.url}`, { status: 404 });
    }
    return buildResponse(route);
  });
}

export function mockFetchHandler(handler: MockFetchHandler): MockFetchController {
  return withMockedFetch(async (request) => {
    const response = await handler(request);
    if (response instanceof Response) {
      return response;
    }
    return new Response(`No mocked response for ${request.url}`, { status: 501 });
  });
}
