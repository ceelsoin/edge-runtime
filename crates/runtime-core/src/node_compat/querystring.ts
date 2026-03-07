function parse(input: string): Record<string, string | string[]> {
  const params = new URLSearchParams(String(input));
  const out: Record<string, string | string[]> = {};

  for (const [key, value] of params.entries()) {
    const current = out[key];
    if (current === undefined) {
      out[key] = value;
    } else if (Array.isArray(current)) {
      current.push(value);
    } else {
      out[key] = [current, value];
    }
  }

  return out;
}

function stringify(obj: Record<string, unknown>): string {
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(obj ?? {})) {
    if (Array.isArray(value)) {
      for (const item of value) params.append(key, String(item));
    } else if (value !== undefined) {
      params.append(key, String(value));
    }
  }
  return params.toString();
}

function escape(input: string): string {
  return encodeURIComponent(String(input));
}

function unescape(input: string): string {
  return decodeURIComponent(String(input));
}

const querystringModule = { parse, stringify, escape, unescape };

export { parse, stringify, escape, unescape };
export default querystringModule;
