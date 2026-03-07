import fs from "node:fs";

function rejectFs(op: string, path?: string) {
  try {
    if (op === "readFile") {
      fs.readFileSync(path ?? "");
    }
    if (op === "writeFile") {
      fs.writeFileSync(path ?? "");
    }
    if (op === "stat") {
      fs.statSync(path ?? "");
    }
    if (op === "lstat") {
      fs.lstatSync(path ?? "");
    }
    if (op === "readdir") {
      fs.readdirSync(path ?? "");
    }
  } catch (e) {
    return Promise.reject(e);
  }
  return Promise.reject(new Error("[edge-runtime] unexpected fs stub state"));
}

function readFile(path: string): Promise<never> {
  return rejectFs("readFile", path);
}

function writeFile(path: string): Promise<never> {
  return rejectFs("writeFile", path);
}

function stat(path: string): Promise<never> {
  return rejectFs("stat", path);
}

function lstat(path: string): Promise<never> {
  return rejectFs("lstat", path);
}

function readdir(path: string): Promise<never> {
  return rejectFs("readdir", path);
}

const fsPromises = {
  readFile,
  writeFile,
  stat,
  lstat,
  readdir,
};

export { readFile, writeFile, stat, lstat, readdir };
export default fsPromises;
