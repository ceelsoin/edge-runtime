type NodeLikeError = Error & { code?: string };

function notImplemented(api: string): never {
  const err = new Error(
    `[edge-runtime] ${api} is not implemented in this runtime profile`,
  ) as NodeLikeError;
  err.code = "ERR_NOT_IMPLEMENTED";
  throw err;
}

function createGzip(): never {
  return notImplemented("zlib.createGzip");
}

function createGunzip(): never {
  return notImplemented("zlib.createGunzip");
}

function gzipSync(): never {
  return notImplemented("zlib.gzipSync");
}

function gunzipSync(): never {
  return notImplemented("zlib.gunzipSync");
}

const constants = {
  Z_NO_FLUSH: 0,
  Z_FINISH: 4,
};

const zlibModule = {
  createGzip,
  createGunzip,
  gzipSync,
  gunzipSync,
  constants,
};

export { createGzip, createGunzip, gzipSync, gunzipSync, constants };
export default zlibModule;
