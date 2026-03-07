type NodeLikeError = Error & { code?: string };

function notImplemented(api: string): never {
  const err = new Error(
    `[edge-runtime] ${api} is not implemented in this runtime profile`,
  ) as NodeLikeError;
  err.code = "ERR_NOT_IMPLEMENTED";
  throw err;
}

function createServer(): never {
  return notImplemented("tls.createServer");
}

function connect(): never {
  return notImplemented("tls.connect");
}

function createSecureContext(): never {
  return notImplemented("tls.createSecureContext");
}

const rootCertificates: string[] = [];

const tlsModule = {
  createServer,
  connect,
  createSecureContext,
  rootCertificates,
};

export { createServer, connect, createSecureContext, rootCertificates };
export default tlsModule;
