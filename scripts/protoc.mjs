#!/usr/bin/env node
// `node scripts/protoc.mjs`: puts the protoc that ehr/build.rs runs into /.protoc/<version>/
// (git-ignored). It is Google's own release, pinned by SHA-256 in ehr/protoc.json
// (DEPENDENCIES.md §5). `npm run dev` and `npm run check` call it before any cargo step, so it
// runs by hand only after a fresh clone, before a plain `cargo build`.
//
// 1. Pick this machine's asset from ehr/protoc.json. No asset: stop and say so.
// 2. Already installed with the same hash: done (no network).
// 3. Download the zip, check its SHA-256 against the pin. A mismatch stops here, before any
//    byte is unpacked.
// 4. Unpack bin/ and include/ into a fresh folder (no entry may leave it), then move the folder
//    into place, so a stopped run never leaves half a protoc.
// No package and no shell: Node's fetch, crypto and zlib only.
import { createHash } from "node:crypto";
import { chmodSync, existsSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { inflateRawSync } from "node:zlib";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const PIN = JSON.parse(readFileSync(join(ROOT, "ehr", "protoc.json"), "utf8"));
const MARK = ".sha256";

/** The folder for the pinned version, and the paths build.rs uses. */
export function protocPaths() {
  const dir = join(ROOT, ".protoc", PIN.version);
  const exe = process.platform === "win32" ? "protoc.exe" : "protoc";
  return { dir, bin: join(dir, "bin", exe), include: join(dir, "include") };
}

/** Install the pinned protoc when it is missing. Returns its paths; throws with the reason. */
export async function ensureProtoc({ quiet = false } = {}) {
  const key = `${process.platform}-${process.arch}`;
  const asset = PIN.assets[key];
  if (!asset) throw new Error(`protoc: ehr/protoc.json has no asset for ${key}`);
  const paths = protocPaths();
  const mark = join(paths.dir, MARK);
  if (existsSync(paths.bin) && existsSync(mark) && readFileSync(mark, "utf8").trim() === asset.sha256) return paths;

  const url = PIN.release + asset.file;
  if (!quiet) console.log(`[protoc] fetching ${asset.file} (Google's release, pinned by SHA-256)`);
  let res;
  try {
    res = await fetch(url, { redirect: "follow", signal: AbortSignal.timeout(60_000) });
  } catch (e) {
    throw new Error(`protoc: no answer from ${url}: ${e.cause?.code ?? e.message}`);
  }
  if (!res.ok) throw new Error(`protoc: ${url} answered ${res.status}`);
  const zip = Buffer.from(await res.arrayBuffer());
  const got = createHash("sha256").update(zip).digest("hex");
  if (got !== asset.sha256) {
    throw new Error(`protoc: ${asset.file} does not match its pin (expected ${asset.sha256}, got ${got}). Nothing was unpacked.`);
  }

  const tmp = `${paths.dir}.tmp`;
  rmSync(tmp, { recursive: true, force: true });
  mkdirSync(tmp, { recursive: true });
  for (const { name, data } of unzip(zip)) {
    if (!name.startsWith("bin/") && !name.startsWith("include/")) continue;
    const out = resolve(tmp, name);
    if (!out.startsWith(resolve(tmp) + sep)) throw new Error(`protoc: the zip names a path outside its folder: ${name}`);
    if (name.endsWith("/")) {
      mkdirSync(out, { recursive: true });
      continue;
    }
    mkdirSync(dirname(out), { recursive: true });
    writeFileSync(out, data);
  }
  const tmpBin = join(tmp, "bin", process.platform === "win32" ? "protoc.exe" : "protoc");
  if (!existsSync(tmpBin)) throw new Error(`protoc: ${asset.file} holds no bin/protoc`);
  if (process.platform !== "win32") chmodSync(tmpBin, 0o755);
  writeFileSync(join(tmp, MARK), `${asset.sha256}\n`);
  rmSync(paths.dir, { recursive: true, force: true });
  renameSync(tmp, paths.dir);
  if (!quiet) console.log(`[protoc] ${PIN.version} installed in .protoc/${PIN.version}`);
  return paths;
}

/** The entries of a zip file: just enough of the format for Google's release zips. */
function unzip(buf) {
  let eocd = -1;
  for (let i = buf.length - 22; i >= Math.max(0, buf.length - 65_557); i--) {
    if (buf.readUInt32LE(i) === 0x06054b50) {
      eocd = i;
      break;
    }
  }
  if (eocd < 0) throw new Error("protoc: the download is not a zip file");
  const count = buf.readUInt16LE(eocd + 10);
  let p = buf.readUInt32LE(eocd + 16);
  const entries = [];
  for (let n = 0; n < count; n++) {
    if (buf.readUInt32LE(p) !== 0x02014b50) throw new Error("protoc: a broken zip directory");
    const method = buf.readUInt16LE(p + 10);
    const size = buf.readUInt32LE(p + 20);
    const nameLen = buf.readUInt16LE(p + 28);
    const extraLen = buf.readUInt16LE(p + 30);
    const commentLen = buf.readUInt16LE(p + 32);
    const local = buf.readUInt32LE(p + 42);
    const name = buf.toString("utf8", p + 46, p + 46 + nameLen);
    p += 46 + nameLen + extraLen + commentLen;
    if (buf.readUInt32LE(local) !== 0x04034b50) throw new Error(`protoc: a broken zip entry: ${name}`);
    const start = local + 30 + buf.readUInt16LE(local + 26) + buf.readUInt16LE(local + 28);
    const raw = buf.subarray(start, start + size);
    let data;
    if (method === 0) data = raw;
    else if (method === 8) data = inflateRawSync(raw);
    else throw new Error(`protoc: zip method ${method} is not supported (${name})`);
    entries.push({ name, data });
  }
  return entries;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const { bin } = await ensureProtoc();
    console.log(`[protoc] ready: ${bin}`);
  } catch (e) {
    console.error(e.message);
    process.exit(1);
  }
}
