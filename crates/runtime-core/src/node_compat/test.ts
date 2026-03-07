type NodeLikeError = Error & { code?: string };

function unsupported(): never {
  const err = new Error(
    "[edge-runtime] node:test is not supported in this runtime profile",
  ) as NodeLikeError;
  err.code = "ERR_NOT_SUPPORTED";
  throw err;
}

function test(): never {
  return unsupported();
}

const it = test;
const describe = test;
const before = test;
const after = test;
const beforeEach = test;
const afterEach = test;

const testModule = {
  test,
  it,
  describe,
  before,
  after,
  beforeEach,
  afterEach,
};

export { test, it, describe, before, after, beforeEach, afterEach };
export default testModule;
