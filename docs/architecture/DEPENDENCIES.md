---
status: foundation
desk_test: none
sources: [core/Cargo.toml, ehr/Cargo.toml, Cargo.lock, deny.toml, agent/pyproject.toml, agent/uv.lock, package.json, package-lock.json, ehr/jwks-worker/package.json, ehr/protoc.json, scripts/protoc.mjs]
updated: 2026-11-16
---

# DeetsHealth — the dependencies

Every crate and package that the project uses directly, with its source, the version that is
locked now, and the newest version. The `archive/` code is not here: it is not built.

**Rule:** a commit that adds, removes or moves a direct dependency to a new major version
changes its row here in the same commit. A refresh of the "Latest" column is a separate
commit (§6).

Terms:
- **Current**: the version in the lock file (`Cargo.lock`, `agent/uv.lock`,
  `package-lock.json`).
- **Latest**: the newest stable release on the day in `updated`. **Bold** = a new major
  version, or a new 0.x minor version. Both can break the code (§4).
- **Range**: what the manifest allows. A refresh inside the range is safe; a bold version is
  outside it.

## 1. Rust (`core/`, `ehr/`)

| Crate | Author / source | Used in | Range | Current | Latest |
|---|---|---|---|---|---|
| jiff | Andrew Gallant (BurntSushi) | ehr | 1.0 | 1.0.0 | 1.0.0 |
| tokio | Tokio project | ehr | 1 | 1.54.1 | 1.54.1 |
| tokio-stream | Tokio project | ehr | 0.1 | 0.1.19 | 0.1.19 |
| tonic | Tokio project (hyperium) | ehr | 0.14 | 0.14.9 | 0.14.9 |
| tonic-prost | Tokio project | ehr | 0.14 | 0.14.9 | 0.14.9 |
| tonic-health | Tokio project | ehr | 0.14 | 0.14.9 | 0.14.9 |
| tonic-prost-build | Tokio project | ehr (build) | 0.14 | 0.14.9 | 0.14.9 |
| prost | Tokio project | ehr | 0.14 | 0.14.6 | 0.14.6 |
| prost-types | Tokio project | ehr (build) | 0.14 | 0.14.6 | 0.14.6 |
| rustls | rustls project (ISRG / Prossimo) | ehr | 0.23 | 0.23.46 | 0.23.46 (0.24 is a dev release) |
| tokio-rustls | rustls project | ehr | 0.26 | 0.26.6 | 0.26.6 |
| rustls-pki-types | rustls project | ehr | 1 | 1.16.0 | 1.16.0 |
| webpki-roots | rustls project (Mozilla's root CAs) | ehr | 1 | 1.0.12 | 1.0.12 |
| aws-lc-rs | AWS | ehr | 1 | 1.19.0 | 1.19.0 |
| rcgen | rustls project | ehr (tests) | 0.14 | 0.14.10 | 0.14.10 |
| reqwest | Sean McArthur | core, ehr | 0.13 | 0.13.5 | 0.13.5 |
| sqlx | its principal authors (was LaunchBadge; moving to `transact-rs`) | ehr | 0.9 | 0.9.0 | 0.9.0 |
| getrandom | rust-random project | ehr | 0.4 | 0.4.3 | 0.4.3 |
| base64 | Marshall Pierce | ehr | 0.23 | 0.23.1 | 0.23.1 |
| serde | David Tolnay | core, ehr | 1 | 1.0.229 | 1.0.229 |
| serde_json | David Tolnay | core, ehr (also ehr's build: the protoc pin) | 1 | 1.0.154 | 1.0.154 |
| zeroize | RustCrypto | core, ehr | 1 | 1.9.1 | 1.9.1 |
| subtle | dalek-cryptography | core | 2 | 2.6.1 | 2.6.1 |
| futures-util | rust-lang (futures-rs) | core | 0.3 | 0.3.34 | 0.3.34 |
| rusqlite | rusqlite project | core (`sqlite` feature, patient app) | 0.32 | 0.32.1 | **0.43.0** |
| ts-rs | Aleph Alpha | core (`ts` feature) | 12 | 12.0.1 | 12.0.1 |

**Build tool:** `protoc`, Google (protocolbuffers/protobuf releases), **36.2**, pinned by SHA-256
in `ehr/protoc.json` (§5, item 3). A new version: change the version and every asset's hash
there, from the digests on the GitHub release page; `build.rs` refuses any other version.

**Check tool:** `cargo-deny`, Embark Studios (EmbarkStudios/cargo-deny on GitHub; crates.io owner
`embark-studios`), **0.20.2**, pinned in `scripts/check.mjs` (`CARGO_DENY`). Config:
`deny.toml`. Install once per machine: `cargo install cargo-deny --version 0.20.2 --locked`.
A new version: change both, and read its notes for config changes.

## 2. Python (`agent/`, locked by uv)

| Package | Author / source | Range | Current | Latest |
|---|---|---|---|---|
| pipecat-ai (anthropic, cartesia, webrtc extras) | Daily | >=1.16,<2 | 1.16.0 | 1.16.0 |
| grpcio | Google (gRPC) | >=1.84,<2 | 1.85.1 | 1.85.1 |
| protobuf | Google | >=6,<7 | 6.33.6 | **7.36.2** (Pipecat needs <7) |
| fastapi | Sebastián Ramírez (tiangolo) | >=0.115,<1 | 0.163.1 | 0.163.1 |
| uvicorn | Encode | >=0.32,<1 | 0.54.0 | 0.54.0 |
| aiohttp | aio-libs | >=3.9,<4 | 3.14.5 | 3.14.5 |
| pyjwt (crypto extra) | José Padilla | >=2.10,<3 | 2.15.1 | 2.15.1 |
| python-dotenv | Saurabh Kumar (theskumar) | >=1 | 1.2.4 | 1.2.4 |
| grpcio-tools (dev) | Google (gRPC) | >=1.70,<2 | 1.81.2 | 1.85.1 (needs protobuf 7) |
| pytest (dev) | pytest-dev | >=8 | 9.2.0 | 9.2.0 |
| pytest-asyncio (dev) | pytest-dev | >=1 | 1.5.0 | 1.5.0 |
| ruff (dev) | Astral | >=0.14 | 0.17.5 | 0.17.5 |

Pipecat brings these. They matter to us, but Pipecat sets their range, not us:

| Package | Author / source | Current | Latest |
|---|---|---|---|
| anthropic | Anthropic | 1.17.0 | **2.8.0** (Pipecat holds it at 1.x) |
| aiortc | aiortc project | 1.16.0 | 1.16.0 |
| websockets | Aymeric Augustin | 17.4 | 17.4 |

There is no Cartesia SDK: Pipecat speaks Cartesia's websocket itself. Pipecat's tree is 76
packages in all. `openai`, `onnxruntime` and `pillow` are required by Pipecat itself (no extra
brings them); `opencv-python-headless` comes with the `webrtc` extra (§5, item 2).

## 3. npm

**Console** (root `package.json`):

| Package | Author / source | Range | Current | Latest |
|---|---|---|---|---|
| @pipecat-ai/client-js | Daily | ^1.16.0 | 1.16.0 | 1.16.0 |
| vite (dev) | Evan You / Vite team | ^6.0.3 | 6.4.6 | **8.3.8** |
| typescript (dev) | Microsoft | ~5.6.2 | 5.6.3 | **7.0.2** (the compiler written in Go) |

**JWKS worker** (`ehr/jwks-worker/package.json`, exact pins):

| Package | Author / source | Current | Latest |
|---|---|---|---|
| cf | Cloudflare | 1.0.0-beta.32 | 1.0.0-beta.32 |
| @cloudflare/vite-plugin | Cloudflare | 2.0.0-beta.v.17 | 2.0.0-beta.v.17 (`beta` tag) |
| vite | Vite team | 8.3.8 | 8.3.8 |
| typescript | Microsoft | 7.0.2 | 7.0.2 |

For `@cloudflare/vite-plugin`, npm's `latest` tag points to the 1.x line. Our 2.0 beta is
newer. Compare against the `beta` tag.

## 4. Major updates not taken

> **Part:** project · 2026-11-16 (verdicts from a trial build in a scratch worktree; none taken yet)

Each one is its own change, with the full check (`npm run check:all`).

| Update | Touches | Trial result | Gain | Verdict |
|---|---|---|---|---|
| sqlx 0.8 → 0.9 | ehr: the call store, settings, users | 11 compile errors, all one kind: a query built at run time (`format!` of a table, role or schema name) | Every `query*()` takes only `&'static str`; runtime SQL needs a visible `AssertSqlSafe`. Closes `settings.rs`'s `table: &str` parameter (any string today). Needs Rust 1.94 (we have 1.96). The repo moves to `transact-rs` | **Taken 2026-11-16.** Fixed text joined with `concat!` (settings' version columns, users' UTC times); every query joined from pieces goes through `ehr/src/sql.rs`'s `Sql`, which takes `&'static str`, numbers and bytes only, never a `String` (the one `AssertSqlSafe` in `ehr/src`); `settings::get` and `save` take `table: &'static str`. The store test joins its database names through its own local helper |
| reqwest 0.12 → 0.13 | core, ehr: every HTTPS call to Epic and Twilio | Builds and all ehr tests pass after one line each: the `form` feature is now opt-in; `rustls-tls-manual-roots-no-provider` is now `rustls-no-provider` | The maintained line. Its redirect fix (0.13.4) does not touch us: both ehr clients set `Policy::none()` and our own rustls config | **Taken 2026-11-16**, with §5 item 6 |
| getrandom 0.3 → 0.4 | ehr: the code by text, session ids, the JWT `jti` | Builds with no code change | 0.4.2 checks `ProcessPrng`'s return value on Windows. 0.4 is already in the tree (`tempfile`), so 0.3 leaves it | **Taken 2026-11-16**; 0.3 is gone from the lock |
| base64 0.22 → 0.23 | ehr | Builds with no code change | Little: `tonic` still holds 0.22, and 0.23 is already in the tree (`hyper-util`) | **Taken 2026-11-16** |
| vite 6 → 8, TypeScript 5.6 → 7 | console build | `tsc` clean; `vite build` passes with no config change, and its own build time fell from 1.1 s to 0.35 s | Speed; one toolchain with the JWKS worker. The dev server never faces the tunnel: the demo serves a built copy (DEMO.md §3.2) | **Take**, after the console work in flight lands; needs a desk test of `npm run dev` (proxy, live reload) and the demo build |
| rusqlite 0.32 → 0.43 | core `sqlite` feature only (patient app) | Not tried | None for the phone agent | **Skip**; it leaves with the patient app's code |
| protobuf 7, anthropic 2.x | agent | — | — | Blocked: Pipecat sets the range |

## 5. Trust review (2026-11-15)

All direct dependencies come from known organizations or long-standing maintainers. No source
is untrustworthy. These items need a second look:

1. **Fixed 2026-11-15: `cf` 1.0.0-beta.19 (JWKS worker) had known high-severity advisories** in
   the packages it bundles: `undici` (a TLS certificate check bypass, cache poisoning across
   origins, several denial-of-service bugs) and `zod` (memory exhaustion). Now beta.32 (and the
   plugin beta.v.17): `npm audit` reports 0 and `npm run build` passes. `cf` is a dev and deploy
   tool. It does not run in the deployed worker, but it runs on the owner's PC with his
   Cloudflare sign-in.
   - The npm name `cf` was created in 2013, long before Cloudflare's CLI. Its publisher now is
     Cloudflare (`wrangler-publisher`, repo `cloudflare/cf`). Keep the exact pin: an old
     version range could install code from an earlier owner.
2. **Reviewed 2026-11-15, kept: Pipecat's tree is the largest surface** (76 Python packages).
   - `openai`, `onnxruntime` and `pillow` are hard requirements of `pipecat-ai`; no extra can
     drop them. Only uv's `override-dependencies` could force them out.
   - What a call loads (the kept replay calls, `test_typed_call.py`, `test_voice.py` and
     `test_dev_server.py`, then `sys.modules`): `onnxruntime` **yes**: Pipecat's turn package
     imports `pipecat.audio.turn.smart_turn.local_smart_turn_v3`, even with Cartesia's turns;
     `pillow` **yes**; `openai` and `cv2` no.
   - A forced removal would break calls or run Pipecat in a shape it does not support. That
     fails "patient experience first". All four come from known publishers (Microsoft, OpenAI,
     the Pillow and OpenCV projects), and `uv.lock` pins every wheel by hash.
   - Check again when Pipecat changes major version, or if it makes these optional.
3. **Fixed 2026-11-15: `protoc-bin-vendored` is gone.** One person repackaged Google's `protoc`
   binaries, and every `ehr` build ran that binary (on Windows, the 32-bit build).
   - Now: Google's own release, 36.2 (the same version, so the same generated code). Pinned by
     SHA-256 in `ehr/protoc.json`; the pins are the digests GitHub publishes for each asset.
   - `scripts/protoc.mjs` fetches it once into `/.protoc/` (git-ignored), checks the hash before
     it unpacks a byte, and keeps every entry inside its folder. Node only, no package.
   - `npm run dev`, `npm run check` and `npm run db:test` run it first. A plain `cargo build` on
     a fresh clone stops with the command to run. `npm run env:check` shows its state.
   - `build.rs` refuses any `protoc` whose version is not the pinned one. A deploy image names
     its own copy with `PROTOC` and `PROTOC_INCLUDE`, under the same version check.
4. **Reviewed 2026-11-15, no change: single-maintainer packages near secrets.**
   - `pyjwt` (the Access token check): `access.py` refuses any `alg` but RS256 before it fetches
     a key, and requires `exp`, `iat`, `iss` and `aud`.
   - `base64` (Rust): encodes only public values (the JWT's parts and signature, the public
     JWK, the `jti`, photos). No private key passes through it, so constant-time encoding is not
     needed.
   - `python-dotenv` (reads `.env`, which holds the Anthropic key): Pipecat requires it too, so
     our own parser would not remove it.
5. **Crypto stays on audited primitives**, as SECURITY.md requires: `aws-lc-rs` (AWS, FIPS
   build via the `fips` feature), `rustls`, `zeroize`, `subtle`.
6. **Fixed 2026-11-16: `ehr`'s Linux build linked OpenSSL it never used.** `core` turned on
   reqwest's `native-tls` (for the patient app's fetch, `core/src/net.rs`), and feature
   unification brought `native-tls` → `openssl-sys` into `ehr`. `ehr` uses only `Shielded`,
   sealing and the pass types from `core`.
   - Now: `native-tls` is `core`'s own default feature, and `ehr` depends on `core` with
     `default-features = false`. `Net::new` sets `min_tls_version` only under that feature
     (reqwest has it only with a TLS backend).
   - Proof: `cargo tree -p deetshealth-ehr -e normal --target all -i <crate>` finds no
     `native-tls`, `hyper-tls`, `openssl-sys` or `ring`. One TLS stack: rustls on `aws-lc-rs`
     (FIPS in the `fips` build). `core` built alone keeps `native-tls` for the patient app.
   - A new crate that uses `core` and needs `Net` turns the feature on by name.

Audits run on 2026-11-15:
- `npm audit` (console): 0 vulnerabilities.
- `npm audit` (JWKS worker): the `cf` findings in item 1; 0 after the bump.
- Rust: not run on 2026-11-15. Since 2026-11-16, cargo-deny in `npm run check` (advisories and
  sources for the workspace, banned crates for `ehr`); its first run found no advisory.
- Python (`pip-audit`): not run yet (§6).

## 6. Refresh this doc

```bash
cargo tree --depth 1 -e normal,build,dev --all-features --prefix none
cargo search <crate> --limit 1
cd agent && uv tree --depth 1 --outdated
npm outdated && npm ls --depth=0
cd ehr/jwks-worker && npm outdated && npm view cf dist-tags
gh release view --repo protocolbuffers/protobuf
```

The audits:

```bash
cargo deny --locked --workspace check advisories sources
cargo deny --locked --manifest-path ehr/Cargo.toml check bans
cd agent && uv export --format requirements-txt --no-hashes --all-groups > req.txt && uvx pip-audit -r req.txt --no-deps
npm audit
```

Safe updates inside the ranges: `cargo update`, `cd agent && uv lock --upgrade && uv sync`,
`npm update`. Stop `npm run dev` first: it locks the `ehr` exe and the agent's venv.
