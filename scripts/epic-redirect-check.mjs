// Does Epic's sandbox know a redirect URI yet? (integrations/EPIC.md §3)
//
//   node scripts/epic-redirect-check.mjs [uri ...]
//
// Sends one authorize request per URI, with the non-production client ID from epic-client.json,
// and reads only where Epic redirects. It never follows the redirect, never signs anyone in, and
// prints no ID: "accepted", or Epic's error number (4 = the sandbox does not know this redirect
// URI; a change on fhir.epic.com takes a while to sync). With no URI, it checks the loopback
// return the app sends (`http://127.0.0.1/callback`) and the same with a random port.

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
let id;
try {
  id = JSON.parse(readFileSync(root + "epic-client.json", "utf8")).nonprod_client_id;
} catch {
  console.error("No epic-client.json at the repo root (EPIC.md §2).");
  process.exit(1);
}
if (!id) {
  console.error("epic-client.json has no nonprod_client_id.");
  process.exit(1);
}

const base = "https://fhir.epic.com/interconnect-fhir-oauth";
const port = 49152 + Math.floor(Math.random() * 16000);
const uris = process.argv.slice(2).length ? process.argv.slice(2) : ["http://127.0.0.1/callback", `http://127.0.0.1:${port}/callback`];

for (const uri of uris) {
  const u = new URL(base + "/oauth2/authorize");
  const params = {
    response_type: "code",
    client_id: id,
    redirect_uri: uri,
    scope: "openid fhirUser launch/patient patient/*.read",
    state: "redirect-check",
    aud: base + "/api/FHIR/R4",
    // A fixed challenge: no sign-in is ever finished, so no verifier is needed.
    code_challenge: "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
    code_challenge_method: "S256",
  };
  for (const [k, v] of Object.entries(params)) u.searchParams.set(k, v);
  try {
    const r = await fetch(u, { redirect: "manual" });
    const loc = r.headers.get("location");
    const err = loc ? new URL(loc, base).searchParams.get("error") : "no redirect";
    console.log(`${uri}  ${err ? "error=" + err : "accepted"}`);
  } catch (e) {
    console.log(`${uri}  request failed (${e.cause?.code ?? e.name})`);
  }
}
