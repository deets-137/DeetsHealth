//! The desktop store: one SQLite file per site (CORE-CRATE.md §5).
//!
//! Plain in the file: table names, row keys (`id`, and a link's `from_id` / `to_id`), a
//! link's table names, and `meta` (the schema and index versions). Everything else is a
//! sealed BLOB, `NOT NULL`: a missing value is sealed as "missing", and padding (seal.rs)
//! gives it the same length as any other short value (docs/architecture/STORE.md §4).
//! SQLite never filters or sorts on a sealed value; core does, in Rust.
//!
//! `secure_delete` is on, so SQLite zeroes the pages it frees.
//!
//! Schema 2 (2026-09-21) is schema 1 with every value re-sealed padded. A schema 1 file is
//! migrated in place when it opens: no value is lost and nothing is fetched.

use super::{CachePass, LinkRow, PatientRow, ResourceRow, RowId, SiteRow, Store, Table, Text, WarningRow};
use crate::platform::Cipher;
use crate::seal::{self, Place};
use crate::{CoreError, Shielded};
use rusqlite::{params_from_iter, types::Value, Connection, OptionalExtension, Transaction, TransactionBehavior};
use std::path::Path;
use std::time::Duration;
use zeroize::Zeroizing;

const SCHEMA_VERSION: i64 = 2;
/// How long a write waits for another connection to the same file (a fetch and the start-up
/// re-index can meet) before it fails.
const BUSY_WAIT: Duration = Duration::from_secs(5);

/// Sealed columns of every resource table, in order (ResourceRow).
const RESOURCE_COLS: [&str; 14] = [
    "fhir_type", "fhir_id", "kind", "code_system", "code", "code_display", "status", "start", "end_", "recorded",
    "last_updated", "canonical", "version", "raw",
];
const PATIENT_COLS: [&str; 4] = ["fhir_id", "name", "birth_date", "sex"];
const SITE_COLS: [&str; 13] = [
    "display_name", "base_url", "client_id", "patient_fhir_id", "smart_config", "capability", "software_version", "scopes",
    "access_token", "refresh_token", "token_expiry", "last_fetch", "last_full_fetch",
];
const WARNING_COLS: [&str; 5] = ["fhir_type", "severity", "code", "text", "at"];
const LINK_COLS: [&str; 3] = ["role", "source_field", "target_ref"];
/// The one row key of the single-row tables (`patient`, `site`).
const ONLY: &str = "00000000000000000000000000000000";

/// Marks inside the sealed bytes: a value, or no value.
const NONE: u8 = 0;
const SOME: u8 = 1;

pub struct SiteStore<C: Cipher> {
    conn: Connection,
    cipher: C,
    key: C::Key,
}

fn io<E>(_: E) -> CoreError {
    CoreError::Io
}

fn sealed_cols(cols: &[&str]) -> String {
    cols.iter().map(|c| format!(", {c} BLOB NOT NULL")).collect()
}

impl<C: Cipher> SiteStore<C> {
    /// Open (or make) one site's file, with that site's key.
    pub fn open(path: &Path, cipher: C, key: C::Key) -> Result<Self, CoreError> {
        let conn = Connection::open(path).map_err(io)?;
        let store = Self { conn, cipher, key };
        store.setup()?;
        Ok(store)
    }

    /// An in-memory store, for tests.
    #[cfg(test)]
    pub(crate) fn in_memory(cipher: C, key: C::Key) -> Result<Self, CoreError> {
        let store = Self { conn: Connection::open_in_memory().map_err(io)?, cipher, key };
        store.setup()?;
        Ok(store)
    }

    fn setup(&self) -> Result<(), CoreError> {
        self.conn.busy_timeout(BUSY_WAIT).map_err(io)?;
        self.conn
            .execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL; PRAGMA secure_delete = ON;")
            .map_err(io)?;
        self.conn
            .execute_batch("CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value INTEGER NOT NULL);")
            .map_err(io)?;
        match self.meta("schema_version")? {
            Some(SCHEMA_VERSION) => Ok(()),
            Some(1) => self.migrate_to_2(),
            // A newer file than this app: never guess at it.
            Some(_) => Err(CoreError::Version),
            None => self.create_schema(),
        }
    }

    fn meta(&self, key: &str) -> Result<Option<i64>, CoreError> {
        self.conn.query_row("SELECT value FROM meta WHERE key = ?", [key], |r| r.get(0)).optional().map_err(io)
    }

    fn set_meta(&self, key: &str, value: i64) -> Result<(), CoreError> {
        self.conn
            .execute("INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)", rusqlite::params![key, value])
            .map_err(io)?;
        Ok(())
    }

    /// Every sealed column, per table: what the migration re-seals.
    fn sealed_tables() -> Vec<(&'static str, &'static [&'static str])> {
        let mut out: Vec<(&str, &[&str])> = Table::ALL.iter().map(|t| (t.name(), &RESOURCE_COLS[..])).collect();
        out.extend([
            ("patient", &PATIENT_COLS[..]),
            ("site", &SITE_COLS[..]),
            ("site_warning", &WARNING_COLS[..]),
            ("link", &LINK_COLS[..]),
        ]);
        out
    }

    /// Schema 1 → 2: re-seal every value padded, in one transaction. The old bytes are freed
    /// pages, which `secure_delete` zeroes; the checkpoint then clears them from the WAL.
    fn migrate_to_2(&self) -> Result<(), CoreError> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate).map_err(io)?;
        // Another connection may have migrated while this one waited for the lock.
        if self.meta("schema_version")? == Some(1) {
            for (table, cols) in Self::sealed_tables() {
                let rows: Vec<(String, Vec<Vec<u8>>)> = {
                    let sql = format!("SELECT id, {} FROM {table}", cols.join(", "));
                    let mut stmt = self.conn.prepare(&sql).map_err(io)?;
                    let rows = stmt
                        .query_map([], |r| {
                            let id: String = r.get(0)?;
                            let v = (0..cols.len()).map(|i| r.get::<_, Vec<u8>>(1 + i)).collect::<Result<Vec<_>, _>>()?;
                            Ok((id, v))
                        })
                        .map_err(io)?
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(io)?;
                    rows
                };
                let sets: Vec<String> = cols.iter().enumerate().map(|(i, c)| format!("{c} = ?{}", i + 1)).collect();
                let sql = format!("UPDATE {table} SET {} WHERE id = ?{}", sets.join(", "), cols.len() + 1);
                for (id, sealed) in rows {
                    let mut params = Vec::with_capacity(cols.len() + 1);
                    for (col, bytes) in cols.iter().zip(&sealed) {
                        let place = Place { table, row: &id, column: col };
                        let plain = seal::open(&self.cipher, &self.key, &place, bytes)?;
                        params.push(Value::Blob(seal::seal(&self.cipher, &self.key, &place, &plain)?));
                    }
                    params.push(Value::Text(id));
                    self.conn.execute(&sql, params_from_iter(params)).map_err(io)?;
                }
            }
            self.set_meta("schema_version", 2)?;
        }
        tx.commit().map_err(io)?;
        // Best effort: a failed checkpoint leaves the old pages in the WAL until the next one.
        let _ = self.conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
        Ok(())
    }

    fn create_schema(&self) -> Result<(), CoreError> {
        let mut sql = String::from("BEGIN;");
        for t in Table::ALL {
            sql += &format!("CREATE TABLE {} (id TEXT PRIMARY KEY{});", t.name(), sealed_cols(&RESOURCE_COLS));
        }
        sql += &format!("CREATE TABLE patient (id TEXT PRIMARY KEY{});", sealed_cols(&PATIENT_COLS));
        sql += &format!("CREATE TABLE site (id TEXT PRIMARY KEY{});", sealed_cols(&SITE_COLS));
        sql += &format!("CREATE TABLE site_warning (id TEXT PRIMARY KEY{});", sealed_cols(&WARNING_COLS));
        sql += &format!(
            "CREATE TABLE link (id TEXT PRIMARY KEY, from_table TEXT NOT NULL, from_id TEXT NOT NULL, \
             to_table TEXT NOT NULL, to_id TEXT{});\
             CREATE INDEX link_from ON link (from_id);\
             CREATE INDEX link_to ON link (to_id);",
            sealed_cols(&LINK_COLS)
        );
        // A new file is empty, so its index is current by definition.
        sql += &format!(
            "INSERT INTO meta (key, value) VALUES ('schema_version', {SCHEMA_VERSION}), ('index_version', {}); COMMIT;",
            crate::index::INDEX_VERSION
        );
        self.conn.execute_batch(&sql).map_err(io)
    }

    /// Run `f` as one transaction: all of its writes land, or none do.
    /// It takes the write lock at the start (IMMEDIATE), so a second connection waits
    /// [`BUSY_WAIT`] for it rather than failing halfway.
    pub fn transaction<R>(&self, f: impl FnOnce(&Self) -> Result<R, CoreError>) -> Result<R, CoreError> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate).map_err(io)?;
        let out = f(self)?;
        tx.commit().map_err(io)?;
        Ok(out)
    }

    // ── sealing one column ──

    fn seal_value(&self, place: &Place, value: Option<&Text>) -> Result<Vec<u8>, CoreError> {
        let pass = CachePass::issue();
        let plain = match value {
            None => Zeroizing::new(vec![NONE]),
            Some(v) => {
                let text = v.reveal(&pass).as_bytes();
                let mut b = Zeroizing::new(Vec::with_capacity(1 + text.len()));
                b.push(SOME);
                b.extend_from_slice(text);
                b
            }
        };
        seal::seal(&self.cipher, &self.key, place, &plain)
    }

    fn open_value(&self, place: &Place, sealed: &[u8]) -> Result<Option<Text>, CoreError> {
        let plain = seal::open(&self.cipher, &self.key, place, sealed)?;
        match plain.split_first() {
            Some((&NONE, [])) => Ok(None),
            Some((&SOME, rest)) => match std::str::from_utf8(rest) {
                Ok(s) => Ok(Some(Shielded::new(s.to_owned()))),
                Err(_) => Err(CoreError::Crypto),
            },
            _ => Err(CoreError::Crypto),
        }
    }

    // ── one generic write and one generic read; every table goes through them ──

    fn write(&self, table: &str, id: &str, plain: &[(&str, Value)], cols: &[&str], values: &[Option<&Text>]) -> Result<(), CoreError> {
        debug_assert_eq!(cols.len(), values.len());
        let mut names = vec!["id"];
        let mut params: Vec<Value> = vec![Value::Text(id.to_owned())];
        for (n, v) in plain {
            names.push(n);
            params.push(v.clone());
        }
        for (col, v) in cols.iter().zip(values) {
            names.push(col);
            params.push(Value::Blob(self.seal_value(&Place { table, row: id, column: col }, *v)?));
        }
        let marks = vec!["?"; names.len()].join(", ");
        let sql = format!("INSERT OR REPLACE INTO {table} ({}) VALUES ({marks})", names.join(", "));
        self.conn.execute(&sql, params_from_iter(params)).map_err(io)?;
        Ok(())
    }

    /// Rows of `table` matching `filter` (plain columns only). Each result: the row key, the
    /// plain columns asked for, and the unsealed columns in `cols` order.
    #[allow(clippy::type_complexity)]
    fn read(
        &self,
        table: &str,
        plain: &[&str],
        cols: &[&str],
        filter: &str,
        args: &[Value],
    ) -> Result<Vec<(String, Vec<Option<String>>, Vec<Option<Text>>)>, CoreError> {
        let names: Vec<&str> = std::iter::once("id").chain(plain.iter().copied()).chain(cols.iter().copied()).collect();
        let sql = format!("SELECT {} FROM {table} {filter}", names.join(", "));
        let mut stmt = self.conn.prepare(&sql).map_err(io)?;
        let raw = stmt
            .query_map(params_from_iter(args.iter()), |r| {
                let id: String = r.get(0)?;
                let p = (0..plain.len()).map(|i| r.get::<_, Option<String>>(1 + i)).collect::<Result<Vec<_>, _>>()?;
                let s = (0..cols.len()).map(|i| r.get::<_, Vec<u8>>(1 + plain.len() + i)).collect::<Result<Vec<_>, _>>()?;
                Ok((id, p, s))
            })
            .map_err(io)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(io)?;
        raw.into_iter()
            .map(|(id, p, sealed)| {
                let open = cols
                    .iter()
                    .zip(&sealed)
                    .map(|(col, b)| self.open_value(&Place { table, row: &id, column: col }, b))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok((id, p, open))
            })
            .collect()
    }

    fn delete(&self, table: &str, id: &str) -> Result<(), CoreError> {
        self.conn.execute(&format!("DELETE FROM {table} WHERE id = ?"), [id]).map_err(io)?;
        Ok(())
    }

    /// Row counts are plain in any SQLite file, so a count reveals nothing new.
    pub fn count(&self, table: Table) -> Result<u64, CoreError> {
        self.conn
            .query_row(&format!("SELECT COUNT(*) FROM {}", table.name()), [], |r| r.get::<_, i64>(0))
            .map(|n| n as u64)
            .map_err(io)
    }

    // ── resources ──

    pub fn put_resource(&self, table: Table, r: &ResourceRow) -> Result<(), CoreError> {
        let v = [
            Some(&r.fhir_type), Some(&r.fhir_id), Some(&r.kind), r.code_system.as_ref(), r.code.as_ref(),
            r.code_display.as_ref(), r.status.as_ref(), r.start.as_ref(), r.end.as_ref(), r.recorded.as_ref(),
            r.last_updated.as_ref(), r.canonical.as_ref(), r.version.as_ref(), Some(&r.raw),
        ];
        self.write(table.name(), r.id.as_str(), &[], &RESOURCE_COLS, &v)
    }

    /// Every row of a table. Core filters and sorts in Rust (CORE-CRATE.md §5).
    pub fn resources(&self, table: Table) -> Result<Vec<ResourceRow>, CoreError> {
        self.read(table.name(), &[], &RESOURCE_COLS, "", &[])?
            .into_iter()
            .map(|(id, _, v)| resource_row(id, v))
            .collect()
    }

    pub fn resource(&self, table: Table, id: &RowId) -> Result<Option<ResourceRow>, CoreError> {
        let rows = self.read(table.name(), &[], &RESOURCE_COLS, "WHERE id = ?", &[Value::Text(id.as_str().to_owned())])?;
        rows.into_iter().next().map(|(id, _, v)| resource_row(id, v)).transpose()
    }

    pub fn delete_resource(&self, table: Table, id: &RowId) -> Result<(), CoreError> {
        self.delete(table.name(), id.as_str())
    }

    // ── the patient and the site (one row each) ──

    pub fn put_patient(&self, p: &PatientRow) -> Result<(), CoreError> {
        let v = [Some(&p.fhir_id), p.name.as_ref(), p.birth_date.as_ref(), p.sex.as_ref()];
        self.write("patient", ONLY, &[], &PATIENT_COLS, &v)
    }

    pub fn patient(&self) -> Result<Option<PatientRow>, CoreError> {
        let mut rows = self.read("patient", &[], &PATIENT_COLS, "", &[])?;
        let Some((_, _, v)) = rows.pop() else { return Ok(None) };
        let mut v = v.into_iter();
        let mut next = || v.next().flatten();
        Ok(Some(PatientRow { fhir_id: next().ok_or(CoreError::Io)?, name: next(), birth_date: next(), sex: next() }))
    }

    pub fn put_site(&self, s: &SiteRow) -> Result<(), CoreError> {
        let v = [
            s.display_name.as_ref(), s.base_url.as_ref(), s.client_id.as_ref(), s.patient_fhir_id.as_ref(), s.smart_config.as_ref(),
            s.capability.as_ref(), s.software_version.as_ref(), s.scopes.as_ref(), s.access_token.as_ref(),
            s.refresh_token.as_ref(), s.token_expiry.as_ref(), s.last_fetch.as_ref(), s.last_full_fetch.as_ref(),
        ];
        self.write("site", ONLY, &[], &SITE_COLS, &v)
    }

    pub fn site(&self) -> Result<Option<SiteRow>, CoreError> {
        let mut rows = self.read("site", &[], &SITE_COLS, "", &[])?;
        let Some((_, _, v)) = rows.pop() else { return Ok(None) };
        let mut v = v.into_iter();
        let mut next = || v.next().flatten();
        Ok(Some(SiteRow {
            display_name: next(),
            base_url: next(),
            client_id: next(),
            patient_fhir_id: next(),
            smart_config: next(),
            capability: next(),
            software_version: next(),
            scopes: next(),
            access_token: next(),
            refresh_token: next(),
            token_expiry: next(),
            last_fetch: next(),
            last_full_fetch: next(),
        }))
    }

    // ── warnings ──

    pub fn add_warning(&self, w: &WarningRow) -> Result<(), CoreError> {
        let v = [Some(&w.fhir_type), w.severity.as_ref(), w.code.as_ref(), w.text.as_ref(), Some(&w.at)];
        self.write("site_warning", w.id.as_str(), &[], &WARNING_COLS, &v)
    }

    pub fn warnings(&self) -> Result<Vec<WarningRow>, CoreError> {
        self.read("site_warning", &[], &WARNING_COLS, "", &[])?
            .into_iter()
            .map(|(id, _, v)| {
                let mut v = v.into_iter();
                let mut next = || v.next().flatten();
                Ok(WarningRow {
                    id: RowId::from_stored(id)?,
                    fhir_type: next().ok_or(CoreError::Io)?,
                    severity: next(),
                    code: next(),
                    text: next(),
                    at: next().ok_or(CoreError::Io)?,
                })
            })
            .collect()
    }

    pub fn clear_warnings(&self) -> Result<(), CoreError> {
        self.conn.execute("DELETE FROM site_warning", []).map_err(io)?;
        Ok(())
    }

    // ── links ──

    pub fn put_link(&self, l: &LinkRow) -> Result<(), CoreError> {
        let plain = [
            ("from_table", Value::Text(l.from_table.name().into())),
            ("from_id", Value::Text(l.from_id.as_str().into())),
            ("to_table", Value::Text(l.to_table.name().into())),
            ("to_id", l.to_id.as_ref().map_or(Value::Null, |t| Value::Text(t.as_str().into()))),
        ];
        let v = [Some(&l.role), Some(&l.source_field), Some(&l.target_ref)];
        self.write("link", l.id.as_str(), &plain, &LINK_COLS, &v)
    }

    /// Links that start at a row.
    pub fn links_from(&self, from: &RowId) -> Result<Vec<LinkRow>, CoreError> {
        self.links("WHERE from_id = ?", &[Value::Text(from.as_str().to_owned())])
    }

    /// Links that end at a row.
    pub fn links_to(&self, to: &RowId) -> Result<Vec<LinkRow>, CoreError> {
        self.links("WHERE to_id = ?", &[Value::Text(to.as_str().to_owned())])
    }

    pub fn delete_links_from(&self, from: &RowId) -> Result<(), CoreError> {
        self.conn.execute("DELETE FROM link WHERE from_id = ?", [from.as_str()]).map_err(io)?;
        Ok(())
    }

    fn links(&self, filter: &str, args: &[Value]) -> Result<Vec<LinkRow>, CoreError> {
        let plain = ["from_table", "from_id", "to_table", "to_id"];
        self.read("link", &plain, &LINK_COLS, filter, args)?
            .into_iter()
            .map(|(id, p, v)| {
                let mut v = v.into_iter();
                let mut next = || v.next().flatten().ok_or(CoreError::Io);
                let table = |s: &Option<String>| {
                    Table::ALL.into_iter().find(|t| Some(t.name()) == s.as_deref()).ok_or(CoreError::Io)
                };
                Ok(LinkRow {
                    id: RowId::from_stored(id)?,
                    from_table: table(&p[0])?,
                    from_id: RowId::from_stored(p[1].clone().ok_or(CoreError::Io)?)?,
                    to_table: table(&p[2])?,
                    to_id: p[3].clone().map(RowId::from_stored).transpose()?,
                    role: next()?,
                    source_field: next()?,
                    target_ref: next()?,
                })
            })
            .collect()
    }
}

impl<C: Cipher> Store for SiteStore<C> {
    fn new_row_id(&self) -> Result<RowId, CoreError> {
        RowId::generate(&self.cipher)
    }
    fn site(&self) -> Result<Option<SiteRow>, CoreError> {
        SiteStore::site(self)
    }
    fn put_site(&self, s: &SiteRow) -> Result<(), CoreError> {
        SiteStore::put_site(self, s)
    }
    fn put_patient(&self, p: &PatientRow) -> Result<(), CoreError> {
        SiteStore::put_patient(self, p)
    }
    /// Reads two sealed columns per row, not the raw resource.
    fn keys(&self, table: Table) -> Result<Vec<(RowId, String, Text)>, CoreError> {
        let pass = CachePass::issue();
        self.read(table.name(), &[], &["fhir_type", "fhir_id"], "", &[])?
            .into_iter()
            .map(|(id, _, v)| {
                let mut v = v.into_iter();
                let t = v.next().flatten().ok_or(CoreError::Io)?;
                let fhir_id = v.next().flatten().ok_or(CoreError::Io)?;
                Ok((RowId::from_stored(id)?, t.reveal(&pass).clone(), fhir_id))
            })
            .collect()
    }
    fn put_resource(&self, table: Table, r: &ResourceRow) -> Result<(), CoreError> {
        SiteStore::put_resource(self, table, r)
    }
    fn resources(&self, table: Table) -> Result<Vec<ResourceRow>, CoreError> {
        SiteStore::resources(self, table)
    }
    fn put_link(&self, l: &LinkRow) -> Result<(), CoreError> {
        SiteStore::put_link(self, l)
    }
    fn delete_links_from(&self, from: &RowId) -> Result<(), CoreError> {
        SiteStore::delete_links_from(self, from)
    }
    fn unresolved_links(&self) -> Result<Vec<LinkRow>, CoreError> {
        self.links("WHERE to_id IS NULL", &[])
    }
    fn set_link_target(&self, link: &RowId, to: &RowId) -> Result<(), CoreError> {
        // `to_id` is a plain row key (it lets SQL join), so no seal is involved.
        self.conn.execute("UPDATE link SET to_id = ?1 WHERE id = ?2", [to.as_str(), link.as_str()]).map_err(io)?;
        Ok(())
    }
    fn add_warning(&self, w: &WarningRow) -> Result<(), CoreError> {
        SiteStore::add_warning(self, w)
    }
    fn clear_warnings(&self) -> Result<(), CoreError> {
        SiteStore::clear_warnings(self)
    }
    fn in_transaction<R>(&self, f: impl FnOnce(&Self) -> Result<R, CoreError>) -> Result<R, CoreError> {
        self.transaction(f)
    }
    fn index_version(&self) -> Result<i64, CoreError> {
        Ok(self.meta("index_version")?.unwrap_or(0))
    }
    fn set_index_version(&self, v: i64) -> Result<(), CoreError> {
        self.set_meta("index_version", v)
    }
}

fn resource_row(id: String, v: Vec<Option<Text>>) -> Result<ResourceRow, CoreError> {
    let mut v = v.into_iter();
    let mut next = || v.next().flatten();
    Ok(ResourceRow {
        id: RowId::from_stored(id)?,
        fhir_type: next().ok_or(CoreError::Io)?,
        fhir_id: next().ok_or(CoreError::Io)?,
        kind: next().ok_or(CoreError::Io)?,
        code_system: next(),
        code: next(),
        code_display: next(),
        status: next(),
        start: next(),
        end: next(),
        recorded: next(),
        last_updated: next(),
        canonical: next(),
        version: next(),
        raw: next().ok_or(CoreError::Io)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::Cipher;

    /// Not a cipher: a stand-in that keeps the associated data next to the bytes and checks
    /// it on open, so these tests cover the store's own logic. The real AES-GCM, and a scan
    /// of a real file for plaintext, are tested in the host (src-tauri/src/cng.rs).
    #[derive(Clone)]
    struct Stand;
    impl Cipher for Stand {
        type Key = ();
        fn seal(&self, _: &(), aad: &[u8], p: &[u8]) -> Result<Vec<u8>, CoreError> {
            let mut v = (aad.len() as u32).to_le_bytes().to_vec();
            v.extend_from_slice(aad);
            v.extend(p.iter().map(|b| b ^ 0x5a));
            Ok(v)
        }
        fn open(&self, _: &(), aad: &[u8], s: &[u8]) -> Result<Zeroizing<Vec<u8>>, CoreError> {
            let n = u32::from_le_bytes(s[..4].try_into().unwrap()) as usize;
            if &s[4..4 + n] != aad {
                return Err(CoreError::Crypto);
            }
            Ok(Zeroizing::new(s[4 + n..].iter().map(|b| b ^ 0x5a).collect()))
        }
        fn random(&self, out: &mut [u8]) -> Result<(), CoreError> {
            use std::sync::atomic::{AtomicU64, Ordering};
            static N: AtomicU64 = AtomicU64::new(1);
            let n = N.fetch_add(1, Ordering::Relaxed).to_le_bytes();
            for (i, b) in out.iter_mut().enumerate() {
                *b = n[i % 8];
            }
            Ok(())
        }
        fn sha256(&self, _: &[u8]) -> Result<[u8; 32], CoreError> {
            Ok([0; 32])
        }
    }

    fn t(s: &str) -> Text {
        Shielded::new(s.to_owned())
    }
    fn show(v: &Text) -> String {
        v.reveal(&CachePass::for_tests()).clone()
    }
    fn store() -> SiteStore<Stand> {
        SiteStore::in_memory(Stand, ()).unwrap()
    }
    fn lab(id: RowId) -> ResourceRow {
        ResourceRow {
            id,
            fhir_type: t("Observation"),
            fhir_id: t("obs-1"),
            kind: t("lab"),
            code_system: Some(t("http://loinc.org")),
            code: Some(t("2345-7")),
            code_display: None,
            status: Some(t("final")),
            start: Some(t("2019-04")),
            end: None,
            recorded: None,
            last_updated: None,
            canonical: None,
            version: None,
            raw: t("{\"resourceType\":\"Observation\"}"),
        }
    }

    #[test]
    fn resource_round_trip_keeps_missing_as_missing() {
        let s = store();
        let id = RowId::generate(&Stand).unwrap();
        s.put_resource(Table::Result, &lab(id.clone())).unwrap();
        let back = s.resource(Table::Result, &id).unwrap().unwrap();
        assert_eq!(show(&back.kind), "lab");
        assert_eq!(show(back.start.as_ref().unwrap()), "2019-04");
        assert!(back.code_display.is_none() && back.end.is_none());
        assert_eq!(s.count(Table::Result).unwrap(), 1);
        assert_eq!(s.count(Table::Medication).unwrap(), 0);
    }

    #[test]
    fn a_value_moved_to_another_row_does_not_open() {
        let s = store();
        let (a, b) = (RowId::generate(&Stand).unwrap(), RowId::generate(&Stand).unwrap());
        s.put_resource(Table::Result, &lab(a.clone())).unwrap();
        s.put_resource(Table::Result, &lab(b.clone())).unwrap();
        s.conn
            .execute("UPDATE result SET kind = (SELECT kind FROM result WHERE id = ?1) WHERE id = ?2", [a.as_str(), b.as_str()])
            .unwrap();
        assert!(s.resource(Table::Result, &a).is_ok());
        assert_eq!(s.resource(Table::Result, &b).err(), Some(CoreError::Crypto));
    }

    #[test]
    fn patient_site_warnings() {
        let s = store();
        assert!(s.patient().unwrap().is_none() && s.site().unwrap().is_none());
        s.put_patient(&PatientRow { fhir_id: t("p1"), name: Some(t("[]")), birth_date: None, sex: Some(t("female")) }).unwrap();
        let p = s.patient().unwrap().unwrap();
        assert_eq!(show(&p.fhir_id), "p1");
        assert!(p.birth_date.is_none());

        s.put_site(&SiteRow { base_url: Some(t("https://sandbox.example/fhir")), ..Default::default() }).unwrap();
        s.put_site(&SiteRow { refresh_token: Some(t("r")), ..Default::default() }).unwrap();
        let site = s.site().unwrap().unwrap();
        assert!(site.base_url.is_none(), "put_site replaces the whole row");
        assert_eq!(show(site.refresh_token.as_ref().unwrap()), "r");

        let w = WarningRow { id: RowId::generate(&Stand).unwrap(), fhir_type: t("Observation"), severity: None, code: Some(t("suppressed")), text: None, at: t("now") };
        s.add_warning(&w).unwrap();
        assert_eq!(s.warnings().unwrap().len(), 1);
        s.clear_warnings().unwrap();
        assert!(s.warnings().unwrap().is_empty());
    }

    #[test]
    fn links_join_on_plain_keys() {
        let s = store();
        let (result, order) = (RowId::generate(&Stand).unwrap(), RowId::generate(&Stand).unwrap());
        let link = |to: Option<RowId>| LinkRow {
            id: RowId::generate(&Stand).unwrap(),
            from_table: Table::Result,
            from_id: result.clone(),
            to_table: Table::Order,
            to_id: to,
            role: t("fulfils"),
            source_field: t("Observation.basedOn"),
            target_ref: t("ServiceRequest/sr-1"),
        };
        s.put_link(&link(Some(order.clone()))).unwrap();
        s.put_link(&link(None)).unwrap();
        let from = s.links_from(&result).unwrap();
        assert_eq!(from.len(), 2);
        assert_eq!(from.iter().filter(|l| l.to_id.is_none()).count(), 1);
        let to = s.links_to(&order).unwrap();
        assert_eq!(to.len(), 1);
        assert_eq!(show(&to[0].role), "fulfils");
        assert!(to[0].to_table == Table::Order);
        s.delete_links_from(&result).unwrap();
        assert!(s.links_from(&result).unwrap().is_empty());
    }

    #[test]
    fn a_transaction_is_all_or_nothing() {
        let s = store();
        let id = RowId::generate(&Stand).unwrap();
        let r = s.transaction(|s| {
            s.put_resource(Table::Result, &lab(id.clone()))?;
            Err::<(), _>(CoreError::Network)
        });
        assert!(r.is_err());
        assert_eq!(s.count(Table::Result).unwrap(), 0);
    }

    #[test]
    fn a_newer_file_is_refused() {
        let dir = std::env::temp_dir().join(format!("dh-store-test-{}", crate::platform::random_hex(&Stand).unwrap()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("site.db");
        {
            let s = SiteStore::open(&path, Stand, ()).unwrap();
            s.conn.execute("UPDATE meta SET value = 99 WHERE key = 'schema_version'", []).unwrap();
        }
        assert_eq!(SiteStore::open(&path, Stand, ()).err(), Some(CoreError::Version));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Padding: a missing value and a short one seal to the same length.
    #[test]
    fn missing_and_short_values_look_the_same() {
        let s = store();
        let place = Place { table: "result", row: "r", column: "end_" };
        let missing = s.seal_value(&place, None).unwrap();
        let short = s.seal_value(&place, Some(&t("2019-04-02"))).unwrap();
        assert_eq!(missing.len(), short.len());
    }

    /// Schema 1 → 2: every value re-sealed padded, none lost, and the index marked stale.
    #[test]
    fn a_schema_1_file_is_migrated_in_place() {
        let dir = std::env::temp_dir().join(format!("dh-store-test-{}", crate::platform::random_hex(&Stand).unwrap()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("site.db");
        let id = RowId::generate(&Stand).unwrap();
        {
            let s = SiteStore::open(&path, Stand, ()).unwrap();
            s.put_resource(Table::Result, &lab(id.clone())).unwrap();
            s.put_site(&SiteRow { refresh_token: Some(t("r")), ..Default::default() }).unwrap();
            // Turn it into a schema 1 file: every value in format 1, with the old markers.
            for (table, cols) in SiteStore::<Stand>::sealed_tables() {
                let ids: Vec<String> = s
                    .conn
                    .prepare(&format!("SELECT id FROM {table}"))
                    .unwrap()
                    .query_map([], |r| r.get(0))
                    .unwrap()
                    .map(Result::unwrap)
                    .collect();
                for row in ids {
                    for col in cols {
                        let place = Place { table, row: &row, column: col };
                        let b: Vec<u8> =
                            s.conn.query_row(&format!("SELECT {col} FROM {table} WHERE id = ?"), [&row], |r| r.get(0)).unwrap();
                        let plain = seal::open(&Stand, &(), &place, &b).unwrap();
                        let old = seal::seal_v1(&Stand, &(), &place, &plain).unwrap();
                        s.conn.execute(&format!("UPDATE {table} SET {col} = ?1 WHERE id = ?2"), rusqlite::params![old, row]).unwrap();
                    }
                }
            }
            s.conn.execute("UPDATE meta SET value = 1 WHERE key = 'schema_version'", []).unwrap();
            s.conn.execute("DELETE FROM meta WHERE key = 'index_version'", []).unwrap();
        }
        let s = SiteStore::open(&path, Stand, ()).unwrap();
        assert_eq!(s.meta("schema_version").unwrap(), Some(2));
        let raw: Vec<u8> = s.conn.query_row("SELECT raw FROM result", [], |r| r.get(0)).unwrap();
        assert!(seal::is_current(&raw));
        let back = s.resource(Table::Result, &id).unwrap().unwrap();
        assert_eq!(show(&back.kind), "lab");
        assert!(back.end.is_none(), "missing stays missing");
        assert_eq!(show(Store::site(&s).unwrap().unwrap().refresh_token.as_ref().unwrap()), "r");
        assert_eq!(Store::index_version(&s).unwrap(), 0, "a migrated file is re-indexed once");
        drop(s);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reindex_runs_only_when_stale() {
        let s = store();
        assert_eq!(Store::index_version(&s).unwrap(), crate::index::INDEX_VERSION, "a new file is current");
        assert!(crate::sync::reindex_if_stale(&s).unwrap().is_none());
        s.set_index_version(0).unwrap();
        assert!(crate::sync::reindex_if_stale(&s).unwrap().is_some());
        assert_eq!(Store::index_version(&s).unwrap(), crate::index::INDEX_VERSION);
    }

    /// Stage 6: rows written with only a raw resource (as stage 5 left them) get their index
    /// columns and links from a re-index, with no fetch; a link to a row that exists points
    /// at it, and a link to a resource never fetched waits.
    #[test]
    fn reindex_builds_index_and_links_from_raw() {
        let s = store();
        let raw_row = |fhir_type: &str, fhir_id: &str, raw: &str| ResourceRow {
            id: RowId::generate(&Stand).unwrap(),
            fhir_type: t(fhir_type),
            fhir_id: t(fhir_id),
            kind: t("unindexed"),
            code_system: None,
            code: None,
            code_display: None,
            status: None,
            start: None,
            end: None,
            recorded: None,
            last_updated: None,
            canonical: None,
            version: None,
            raw: t(raw),
        };
        let enc = raw_row("Encounter", "e1", r#"{"resourceType":"Encounter","id":"e1","period":{"start":"2020-01-02"}}"#);
        let obs = raw_row(
            "Observation",
            "o1",
            r#"{"resourceType":"Observation","id":"o1","status":"final","category":[{"coding":[{"code":"laboratory"}]}],
               "code":{"coding":[{"system":"https://loinc.org","code":"2345-7"}]},"effectiveDateTime":"2020-01-02",
               "encounter":{"reference":"Encounter/e1"},"basedOn":[{"reference":"ServiceRequest/never-fetched"}]}"#,
        );
        let report = raw_row(
            "DiagnosticReport",
            "r1",
            r#"{"resourceType":"DiagnosticReport","id":"r1","result":[{"reference":"Observation/o1"}]}"#,
        );
        s.put_resource(Table::Encounter, &enc).unwrap();
        s.put_resource(Table::Result, &obs).unwrap();
        s.put_resource(Table::Result, &report).unwrap();

        let done = crate::sync::reindex(&s).unwrap();
        assert_eq!((done.rows, done.links, done.waiting), (3, 3, 1));

        let o = s.resource(Table::Result, &obs.id).unwrap().unwrap();
        assert_eq!(show(&o.kind), "laboratory");
        assert_eq!(show(o.code_system.as_ref().unwrap()), "http://loinc.org");
        assert_eq!(show(o.start.as_ref().unwrap()), "2020-01-02");

        let from_obs = s.links_from(&obs.id).unwrap();
        let collected = from_obs.iter().find(|l| show(&l.role) == "collected").unwrap();
        assert!(collected.to_id.as_ref() == Some(&enc.id), "the result points at its encounter row");
        let fulfils = from_obs.iter().find(|l| show(&l.role) == "fulfils").unwrap();
        assert!(fulfils.to_id.is_none() && fulfils.to_table == Table::Order, "the order was never fetched: it waits");
        let contains = s.links_to(&obs.id).unwrap();
        assert!(contains.len() == 1 && contains[0].from_id == report.id && show(&contains[0].role) == "contains");

        // A second re-index replaces the links; it does not add them.
        crate::sync::reindex(&s).unwrap();
        assert_eq!(s.links_from(&obs.id).unwrap().len(), 2);
    }
}
