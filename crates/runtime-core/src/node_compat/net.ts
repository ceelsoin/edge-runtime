import { EventEmitter } from "node:events";

type NodeLikeError = Error & { code?: string };

function notImplemented(api: string): never {
  const err = new Error(
    `[edge-runtime] ${api} is not implemented in this runtime profile`,
  ) as NodeLikeError;
  err.code = "ERR_NOT_IMPLEMENTED";
  throw err;
}

class Socket extends EventEmitter {
  connect(): never {
    return notImplemented("net.Socket.connect");
  }

  end(): this {
    this.emit("close");
    return this;
  }
}

class Server extends EventEmitter {
  listen(): never {
    return notImplemented("net.Server.listen");
  }

  close(cb?: () => void): this {
    if (typeof cb === "function") cb();
    this.emit("close");
    return this;
  }
}

function createServer(): Server {
  return new Server();
}

function connect(): never {
  return notImplemented("net.connect");
}

const createConnection = connect;

const netModule = { Socket, Server, createServer, connect, createConnection };

export { Socket, Server, createServer, connect, createConnection };
export default netModule;
