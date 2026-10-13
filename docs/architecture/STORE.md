---
status: built
desk_test: none
sources: [core/src/store/mod.rs, core/src/store/sqlite.rs, core/src/seal.rs, src-tauri/src/sites.rs, src-tauri/src/keystore.rs, src-tauri/src/cng.rs]
updated: 2026-09-24
---
# DeetsHealth — The sealed store

> Where a site's record lives on disk, what the file holds, what it shows, and how it is
> erased. The design is DATA-MODEL.md (what goes where) and CORE-CRATE.md §5 (sealing and
> keys); the build history is CORE-CRATE.md §12 (stages 2–3). The rules are SECURITY.md §4.6
> and §4.7. What fills the rows is [FETCH.md](FETCH.md) and [INDEX-AND-LINKS.md](INDEX-AND-LINKS.md).

## 1. Terms
- **Site key** — the app's own random name for one connected site: 32 hex digits, made at the
  first sign-in (`SiteKey`). It names the site's files. It is not the site's name.
- **Data key** — the AES-256 key that seals one site's values. One per site.
- **Row key** — a row's own random name: 32 hex digits (`RowId`). Plain in the file.
- **Sealed value** — one value, encrypted on its own and bound to its table, row and column.
- **Store** — one site's SQLite file, opened with that site's data key (`SiteStore`).

## 2. The files on disk
The app data folder is `%APPDATA%\com.deetshealth.dev\` (a debug build) or
`%APPDATA%\com.deetshealth.app\` (installed).

| Path | What | Protected data? |
|---|---|---|
| `sites\<site key>.db` | The site's store (§3). | Yes, every value sealed. |
| `sites\<site key>.db-wal`, `-shm` | SQLite's side files while the store is open. | Yes, sealed as in the file. |
| `keys\<site key>.key` | The site's data key, wrapped by DPAPI (§5.3). | A credential. |
| `deetshealth.log`, `deetshealth.1.log` | The rolling log (about 1 MB at most). | No: counts, type names and error kinds only. |
| `settings.json` | The back end's settings: the window's position only. | No. Nothing protected may go here. |

- A file name says nothing about the site: the site key is random. `SiteFiles::list` finds
  the connected sites by their `.db` files.
- `*.db`, `*.db-wal`, `*.db-shm` and `*.key` are git-ignored by pattern.
- The folder is under the Roaming profile. On a PC with a domain roaming profile, Windows
  copies it to the domain's server. The key there still needs the same Windows user.

## 3. The schema
`core/src/store/sqlite.rs`, `SCHEMA_VERSION` 2 (schema 1 with every value padded; §6). Every table has `id TEXT PRIMARY KEY` (the
row key). Every other column is a sealed `BLOB NOT NULL` unless the table says **plain**.

| Table | Rows | Sealed columns |
|---|---|---|
| `encounter`, `result`, `medication`, `document`, `order_`, `chart_item`, `party`, `billing`, `drug`, `other`, `definition` | one per FHIR resource (INDEX-AND-LINKS.md §3) | `fhir_type`, `fhir_id`, `kind`, `code_system`, `code`, `code_display`, `status`, `start`, `end_`, `recorded`, `last_updated`, `canonical`, `version`, `raw` |
| `patient` | one (row key all zeros) | `fhir_id`, `name` (the `HumanName` list as JSON), `birth_date`, `sex` |
| `site` | one (row key all zeros) | `display_name`, `base_url`, `client_id`, `patient_fhir_id`, `smart_config`, `capability`, `software_version`, `scopes`, `access_token`, `refresh_token`, `token_expiry`, `last_fetch`, `last_full_fetch` |
| `site_warning` | one per issue (FETCH.md §7) | `fhir_type`, `severity`, `code`, `text`, `at` |
| `link` | one per reference (INDEX-AND-LINKS.md §7–§9) | `role`, `source_field`, `target_ref`; **plain:** `from_table`, `from_id`, `to_table`, `to_id` (empty while the link waits); indexes `link_from`, `link_to` |
| `meta` | `schema_version`, `index_version` (INDEX-AND-LINKS.md §10) | **plain:** `key`, `value` |

- `order_` and `end_` carry a `_` because `order` and `end` are SQL keywords.
- `raw` is the resource as the site sent it, JSON, in the site's key order. The patient has
  no `raw`: it is trimmed at the parse (FHIR-SHAPES.md §5, fork 1).
- Times of our own (`last_fetch`, `at`) are seconds since 1970, as text. Dates from the site
  are the FHIR text as sent.

## 4. What the file shows
Someone with the file and without the key can read only this:
- the table names, and so which shapes of data the app keeps;
- the row count of each table (how many results, how many visits);
- every row key (random, with no meaning);
- the link graph: which row points to which, and the two tables of each link (so SQLite can
  join). It shows, for example, how many results tie to each encounter row, but not what
  any of them is;
- a **size class** for each sealed value (§5.1): every value under 60 bytes of text (a
  missing value, a date, a code, a system URL) has one length; a longer one (a display text,
  a note, a raw resource) shows only a rough size, within about 12%;
- the schema version and the index version.

It shows no value, no date, no code, no name and no site, and not which values are missing.
**History:** before padding (schema 1), a sealed value was its plaintext length plus 30 bytes,
so a missing value was always the shortest and the file showed which fields a record lacked.
The owner chose padding on 2026-09-21 (Padmé, 64-byte floor).

## 5. Sealing
### 5.1 One value
`SiteStore::seal_value` and `seal::seal`:
1. The value gets a marker byte: `1` then the text, or `0` alone for a missing value.
2. **Padding** (`seal::pad`): a 4-byte length, the bytes, then zeros up to
   `seal::padded_len`: at least 64 bytes (`PAD_FLOOR`), and above that the Padmé rule (keep
   the top bits of the length, round the rest up; at most about 12% extra). The 1,000
   lengths from 1,000 to 1,999 fall into 17 classes.
3. The cipher seals it with AES-256-GCM (Windows CNG) and a fresh random 96-bit nonce. Its
   output is nonce (12) ‖ ciphertext ‖ tag (16).
4. A format byte (`2`) goes in front, so a later algorithm can tell old values from new.
   Format `1` (no padding, schema 1) is still opened, for the migration; it is never
   written.
5. The associated data is `deetshealth.cache.v1`, the table, the row key and the column,
   each with a 4-byte length in front. A value copied to another row or column fails to open.

Sealed length = 1 + 12 + padded length + 16: 93 bytes for every short value.
Cost: about +500 bytes per row against schema 1, and at most about 12% on a raw resource.

### 5.2 Opening
`SiteStore::open_value` reverses it and puts the text straight into `Shielded<String>`. A
wrong key, a wrong place, a changed byte, an unknown format byte, a length past the end, a
padding byte that is not zero, or bytes that are not UTF-8 all give `CoreError::Crypto`. The plaintext buffer is wiped when it drops.

### 5.3 The data key
`src-tauri/src/keystore.rs` (`DpapiKeyStore`) fills the `KeyStore` trait:
- **Create:** 32 bytes from `BCryptGenRandom`, wrapped by `CryptProtectData` for the current
  Windows user with the app's entropy (`deetshealth.sitekey.v1`) and an empty description
  (DPAPI stores the description in the clear). Written whole: a temp file, flush, rename.
  It never replaces a key that exists.
- **Open:** unwraps the blob into a CNG key handle; the raw bytes are wiped at once.
- **Destroy:** overwrites the file, then deletes it.

## 6. Reading and writing
- **One generic write and one generic read** (`write`, `read`) seal and open every column of
  every table. A write is `INSERT OR REPLACE`: it replaces the whole row.
- `put_site` replaces the whole site row. A caller reads the row, changes it and writes all
  of it.
- **SQLite never filters or sorts on a sealed value.** The only `WHERE` clauses are on row
  keys (`id`, `from_id`, `to_id`). Core reads a whole table and filters and sorts in Rust.
- **Finding a row by FHIR id:** `keys(table)` opens two columns per row (`fhir_type`,
  `fhir_id`), not the raw resource; the fetch keeps them in a map (`sync::Keys`). Measured
  under 1 ms on the SMART sandbox.
- **Transactions:** `transaction(f)` (and `in_transaction` in the trait) commits all of
  `f`'s writes or none. It takes the write lock at the start (`IMMEDIATE`). The fetch writes
  each page in one.
- **Settings:** WAL journal, `synchronous = NORMAL`, `secure_delete = ON` (SQLite zeroes the
  pages it frees), and a 5 s busy wait (`BUSY_WAIT`), so a fetch and the start-up check on
  the same file wait for each other instead of failing.
- **Schema version:** version 2 opens. No version: the schema is made, with the index
  marked current. Version 1 is **migrated in place** (`migrate_to_2`): in one transaction,
  every sealed value of every table is opened and sealed again, padded; then a WAL
  checkpoint clears the old pages. No value is lost and nothing is fetched. The index
  version is left unset, so the file is re-indexed once. Any other version is refused
  (`CoreError::Version`).

## 7. The `Store` trait
`core/src/store/mod.rs`. What the fetch and the re-index need from a store. `SiteStore`
fills it on the desktop; a web version fills it over IndexedDB. Every value in and out is
shielded.

| Method | Job |
|---|---|
| `new_row_id` | A fresh row key. |
| `site`, `put_site` | The site row. |
| `put_patient` | The patient row. |
| `keys(table)` | Every row's key, FHIR type and FHIR id. |
| `type_counts(table)` | Rows per FHIR type, for the Sites row. `SiteStore` opens one column (`fhir_type`); the default reads `keys`. |
| `put_resource`, `resources(table)` | Write one resource row; read every row of a table, `raw` included. |
| `put_link`, `delete_links_from`, `unresolved_links`, `set_link_target` | The links. |
| `add_warning`, `clear_warnings` | The site's warnings. |
| `in_transaction(f)` | All or nothing. |
| `index_version`, `set_index_version` | The index rules the rows were built with; 0 when unknown (INDEX-AND-LINKS.md §10). |

`SiteStore` also has, outside the trait: `count(table)`, `resource(table, id)`,
`delete_resource`, `patient`, `warnings`, `links_from(row)`, `links_to(row)`.

The pass: the store is one of the three modules that may read a `Shielded` value
(`CachePass`, SECURITY.md §4.2). It reveals a value only to seal it.

## 8. Erase
`SiteFiles::erase(site)` in `src-tauri/src/sites.rs`. The caller closes the store first.
1. The data key is destroyed. From this moment the file cannot be opened.
2. The `.db`, `-wal` and `-shm` files are deleted.

This is the cryptographic erase of NIST SP 800-88 (SECURITY.md §8.3). A copy of the file made
before the erase cannot be opened: no other key opens it.

**Called by Sign out** (stage 7, 2026-09-24): Settings › Sites › a row's **Sign out**,
pressed twice within 3 s, calls `sites::site_sign_out`. It takes the site's `Running` claim,
so it never erases under a fetch (`sign-out-busy`), and then calls `erase`. A file whose
key is gone still gets a Sites row with Sign out, so an erase that stopped after step 1 can
be finished from the app. The uninstaller's delete of the whole folder (SECURITY.md §4.7)
comes with the release path.

## 9. When it goes wrong
| Symptom | Likely cause | Fix |
|---|---|---|
| `site: the key is gone` (toast `site-key-missing`) | The key file is gone. | Sign out and connect again. The file cannot be saved without its key. |
| `site: the store would not open: key store error` (toast `site-unreadable`) | DPAPI will not unwrap the key (another Windows user, or a profile copied to another PC). | The same. |
| `crypto error` on a read or at the migration | A sealed value was changed or moved, or the file was opened with another site's key. | The same. |
| `written by a newer version` (toast `site-newer`) | A newer build wrote the file. | Use the newer build, or sign out and connect again. |
| A `.db` file with no `.key` | An erase stopped after step 1. | Its Sites row reads "cannot be opened": press Sign out twice. |

## 10. What to change, and where
| To change | Where |
|---|---|
| A column of the resource tables | `RESOURCE_COLS`, `ResourceRow`, `put_resource`, `resource_row`; then raise `SCHEMA_VERSION` and write a migration |
| The site or patient columns | `SITE_COLS` / `PATIENT_COLS` and their row types, the same way |
| A new table | `Table` and `Table::ALL`, `Table::name`; `fhir::table_for` |
| What is plain | `put_link` (the plain columns) and this doc's §4; SECURITY.md §4.6 first |
| The folder layout | `SiteFiles::new` |
| The sealed format or the padding | `seal.rs` (`PAD_FLOOR`, `padded_len`; a new format byte, and keep opening the old ones) |

## 11. The web, later
The row types, the `Store` trait and the seal format are platform-free. A web store fills
the trait over IndexedDB with a non-extractable WebCrypto key. `Cipher` and `KeyStore` are
synchronous today; WebCrypto is async, so the web needs async forms of both
(CORE-CRATE.md §12).

## 12. Tests
- `cargo test -p deetshealth-core --features sqlite` (`store::sqlite::tests`, with a
  stand-in cipher): a round trip that keeps "missing" as missing; a missing and a short
  value seal to the same length; a value moved to another row does not open; the patient,
  site and warning rows; links join on plain keys; a transaction is all or nothing; a newer
  file is refused; **a schema 1 file is migrated in place** (every value current, none lost,
  the index marked stale); the re-index runs only when stale.
- `seal::tests`: format 2 round trip; one sealed length under the floor; Padmé overhead
  under 12% and 17 classes for 1,000–1,999; bad padding refused; format 1 still opens.
- `cargo test -p deetshealth --lib` (the real CNG and DPAPI): **a byte scan of the real
  `.db` and `-wal` files, open and closed, for a marker written into every sealed column:
  not found**; after an erase, a copy of the file taken before cannot be opened; the GCM
  test vectors; DPAPI create, open and destroy.
