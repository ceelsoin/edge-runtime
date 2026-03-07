function getBufferCtor() {
  const ctor = globalThis.Buffer;
  if (typeof ctor !== "function") {
    throw new Error("[edge-runtime] globalThis.Buffer is not available");
  }
  return ctor;
}

export const Buffer = new Proxy(function EdgeBufferProxy() {}, {
  get(_target, prop) {
    return getBufferCtor()[prop];
  },
  set(_target, prop, value) {
    getBufferCtor()[prop] = value;
    return true;
  },
  apply(_target, thisArg, args) {
    return Reflect.apply(getBufferCtor(), thisArg, args);
  },
  construct(_target, args, newTarget) {
    return Reflect.construct(getBufferCtor(), args, newTarget);
  },
});
export const INSPECT_MAX_BYTES = 50;
export const kMaxLength = 0x7fffffff;
export const kStringMaxLength = 0x1fffffe8;

const bufferModule = {
  Buffer,
  INSPECT_MAX_BYTES,
  kMaxLength,
  kStringMaxLength,
};

export default bufferModule;
