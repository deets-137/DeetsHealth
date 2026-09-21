// One SQLite file and one key per site (SECURITY.md §4.6, CORE-CRATE.md §5).
//
//   <app data>/sites/<site key>.db   the sealed store (plus SQLite's -wal / -shm while open)
//   <app data>/keys/<site key>.key   the site's key, wrapped by DPAPI
//
// A site key is the app's own random name (core's SiteKey), so no file name says which
// health system a user signed in to. Erase destroys the key first: from that moment the
// file cannot be opened, whatever happens to the file delete after it.

use crate::cng::Cng;
use crate::keystore::DpapiKeyStore;
use deetshealth_core::platform::{KeyStore, SiteKey};
use deetshealth_core::store::SiteStore;
use deetshealth_core::CoreError;
use std::path::{Path, PathBuf};

pub struct SiteFiles {
    sites: PathBuf,
    keys: DpapiKeyStore,
}

impl SiteFiles {
    pub fn new(app_data: &Path) -> Self {
        Self { sites: app_data.join("sites"), keys: DpapiKeyStore::new(app_data.join("keys")) }
    }

    fn db(&self, site: &SiteKey) -> PathBuf {
        self.sites.join(format!("{}.db", site.as_str()))
    }

    /// A new site at its first sign-in: a new name, a new key, an empty store.
    pub fn create(&self) -> Result<(SiteKey, SiteStore<Cng>), CoreError> {
        std::fs::create_dir_all(&self.sites).map_err(|_| CoreError::Io)?;
        let site = SiteKey::generate(&Cng)?;
        let key = self.keys.create(&site)?;
        let store = SiteStore::open(&self.db(&site), Cng, key)?;
        Ok((site, store))
    }

    /// An existing site's store. `None` if its key is gone (erased, or never made).
    pub fn open(&self, site: &SiteKey) -> Result<Option<SiteStore<Cng>>, CoreError> {
        match self.keys.open(site)? {
            Some(key) => SiteStore::open(&self.db(site), Cng, key).map(Some),
            None => Ok(None),
        }
    }

    /// Every site with a store file, by its app name.
    pub fn list(&self) -> Vec<SiteKey> {
        let Ok(dir) = std::fs::read_dir(&self.sites) else { return Vec::new() };
        dir.filter_map(|e| {
            let name = e.ok()?.file_name().into_string().ok()?;
            SiteKey::from_stored(name.strip_suffix(".db")?.to_owned()).ok()
        })
        .collect()
    }

    /// The erase at sign-out (SECURITY.md §8.3). The caller drops the site's store first.
    /// The key goes first; then the file and SQLite's side files.
    pub fn erase(&self, site: &SiteKey) -> Result<(), CoreError> {
        self.keys.destroy(site)?;
        let db = self.db(site);
        for suffix in ["", "-wal", "-shm"] {
            let p = PathBuf::from(format!("{}{suffix}", db.display()));
            match std::fs::remove_file(&p) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return Err(CoreError::Io),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deetshealth_core::store::{LinkRow, PatientRow, ResourceRow, RowId, SiteRow, Table};
    use deetshealth_core::Shielded;

    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!("deetshealth-sites-{}", SiteKey::generate(&Cng).unwrap().as_str()));
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    const MARK: &str = "SANDBOXMARK7f3a";
    fn t(s: &str) -> Shielded<String> {
        Shielded::new(format!("{MARK}-{s}"))
    }

    /// Every byte SQLite wrote for a site: the file and its side files.
    fn bytes_on_disk(files: &SiteFiles, site: &SiteKey) -> Vec<u8> {
        let db = files.db(site);
        ["", "-wal", "-shm"]
            .iter()
            .filter_map(|s| std::fs::read(format!("{}{s}", db.display())).ok())
            .flatten()
            .collect()
    }

    #[test]
    fn no_plaintext_reaches_the_disk() {
        let dir = TempDir::new();
        let files = SiteFiles::new(&dir.0);
        let (site, store) = files.create().unwrap();
        let id = RowId::generate(&Cng).unwrap();
        store
            .put_resource(
                Table::Result,
                &ResourceRow {
                    id: id.clone(),
                    fhir_type: t("type"),
                    fhir_id: t("id"),
                    kind: t("kind"),
                    code_system: Some(t("system")),
                    code: Some(t("code")),
                    code_display: Some(t("display")),
                    status: Some(t("status")),
                    start: Some(t("start")),
                    end: None,
                    recorded: None,
                    last_updated: None,
                    canonical: None,
                    version: None,
                    raw: t("raw"),
                },
            )
            .unwrap();
        store.put_patient(&PatientRow { fhir_id: t("pid"), name: Some(t("name")), birth_date: Some(t("dob")), sex: None }).unwrap();
        store.put_site(&SiteRow { display_name: Some(t("site")), refresh_token: Some(t("token")), ..Default::default() }).unwrap();
        store
            .put_link(&LinkRow {
                id: RowId::generate(&Cng).unwrap(),
                from_table: Table::Result,
                from_id: id.clone(),
                to_table: Table::Encounter,
                to_id: None,
                role: t("role"),
                source_field: t("field"),
                target_ref: t("ref"),
            })
            .unwrap();

        // While open (the WAL holds the writes) and after close (checkpointed into the file).
        let open_bytes = bytes_on_disk(&files, &site);
        drop(store);
        let closed_bytes = bytes_on_disk(&files, &site);
        for bytes in [&open_bytes, &closed_bytes] {
            assert!(bytes.len() > 4096, "the file was written");
            assert!(!bytes.windows(MARK.len()).any(|w| w == MARK.as_bytes()), "plaintext found on disk");
        }

        // Reopened with the site's key, every value comes back.
        let store = files.open(&site).unwrap().expect("store");
        assert_eq!(store.count(Table::Result).unwrap(), 1);
        assert!(store.resource(Table::Result, &id).unwrap().is_some());
        assert_eq!(store.links_from(&id).unwrap().len(), 1);
        drop(store);
    }

    #[test]
    fn erase_leaves_nothing_that_opens() {
        let dir = TempDir::new();
        let files = SiteFiles::new(&dir.0);
        let (site, store) = files.create().unwrap();
        store.put_site(&SiteRow { display_name: Some(t("site")), ..Default::default() }).unwrap();
        drop(store);
        assert_eq!(files.list().len(), 1);

        // A copy made before sign-out cannot be opened once the key is gone.
        let copy = dir.0.join("copy.db");
        std::fs::copy(files.db(&site), &copy).unwrap();

        files.erase(&site).unwrap();
        assert!(files.list().is_empty());
        assert!(files.open(&site).unwrap().is_none());
        assert!(!files.db(&site).exists());
        // The only key that could open the copy was the erased one; a fresh key cannot.
        let other = DpapiKeyStore::new(dir.0.join("other"));
        let stranger = SiteKey::generate(&Cng).unwrap();
        let wrong = other.create(&stranger).unwrap();
        let store = SiteStore::open(&copy, Cng, wrong).unwrap();
        assert!(store.site().is_err());
    }
}
