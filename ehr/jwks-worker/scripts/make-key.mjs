// Makes the ehr service's Epic signing key for the sandbox (EPIC.md §2, row 3).
//   node scripts/make-key.mjs
// - The private key (P-384, for ES384) goes to <repo>/dev-certs/epic-nonprod-es384.key,
//   which git ignores. It is never printed.
// - The public key goes into src/nonprod.jwks.json, which the Worker serves and git keeps.
// - The kid is the key's RFC 7638 thumbprint, so the JWT header names the key exactly.
// It refuses to replace an existing private key: a new key is a rotation, done by hand
// (add the new public key to the set, move the service over, then drop the old one).
import { generateKeyPairSync, createHash } from "node:crypto";
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const repo = join(here, "..", "..", "..");
const keyPath = join(repo, "dev-certs", "epic-nonprod-es384.key");
const setPath = join(here, "..", "src", "nonprod.jwks.json");

if (existsSync(keyPath)) {
	console.error("A sandbox key already exists. Rotate it by hand; see the note at the top of this script.");
	process.exit(1);
}

const { privateKey, publicKey } = generateKeyPairSync("ec", { namedCurve: "P-384" });
const jwk = publicKey.export({ format: "jwk" });
// RFC 7638: the members crv, kty, x, y in that order, no spaces.
const thumb = createHash("sha256")
	.update(JSON.stringify({ crv: jwk.crv, kty: jwk.kty, x: jwk.x, y: jwk.y }))
	.digest("base64url");

mkdirSync(dirname(keyPath), { recursive: true });
writeFileSync(keyPath, privateKey.export({ format: "pem", type: "pkcs8" }), { mode: 0o600, flag: "wx" });
const set = { keys: [{ kty: jwk.kty, crv: jwk.crv, x: jwk.x, y: jwk.y, kid: thumb, alg: "ES384", use: "sig" }] };
writeFileSync(setPath, JSON.stringify(set, null, "\t") + "\n");

console.log(`Made the sandbox key. kid ${thumb}`);
console.log("Private key: dev-certs/epic-nonprod-es384.key (git-ignored). Public set: src/nonprod.jwks.json.");
