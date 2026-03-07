function toASCII(input: string): string {
  try {
    return new URL(`http://${input}`).hostname;
  } catch {
    return String(input);
  }
}

function toUnicode(input: string): string {
  try {
    return new URL(`http://${input}`).hostname;
  } catch {
    return String(input);
  }
}

function encode(input: string): string {
  return toASCII(input);
}

function decode(input: string): string {
  return toUnicode(input);
}

const ucs2 = {
  decode(input: string) {
    return Array.from(String(input)).map((ch) => ch.codePointAt(0) ?? 0);
  },
  encode(codePoints: number[]) {
    return String.fromCodePoint(...codePoints);
  },
};

const punycodeModule = {
  version: "2.1.1-edge",
  toASCII,
  toUnicode,
  encode,
  decode,
  ucs2,
};

export const version = "2.1.1-edge";
export { toASCII, toUnicode, encode, decode, ucs2 };
export default punycodeModule;
