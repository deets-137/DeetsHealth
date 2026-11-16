//! The call store against a real Postgres (DATABASE.md). Skipped unless `DEETS_DATABASE_URL` is
//! set (`npm run db:test` sets it). Each run makes its own database and drops it after, so the
//! dev data is never touched.

use deetshealth_core::seal::{open_text, Place};
use deetshealth_ehr::{
    seal_key::{AwsLc, SealKey},
    settings,
    store::{DataClass, Store, StoreKeys},
};
use serde_json::json;
use sqlx::{AssertSqlSafe, Connection, PgConnection};

/// SQL this test joins at run time: a database name from its own process id, a table from
/// `sealed_in`. Test code only; ehr's own code joins SQL through `ehr/src/sql.rs`.
fn sql(text: String) -> AssertSqlSafe<String> {
    AssertSqlSafe(text)
}

const RECORD: &str = r#"{"t_ms": 0, "kind": "call_start", "schema_version": 3, "source": "typed", "started": "2026-11-12T13:00:00"}
{"t_ms": 10, "kind": "agent", "text": "Hello, you've reached Main Street Clinic."}
{"t_ms": 20, "kind": "caller", "text": "Jordan Ellis, March 12, 1984"}
{"t_ms": 30, "kind": "staff_flag", "flag": "possible_emergency", "source": "code", "summary": null, "evidence": ["chest pains"], "unverified": 0}
{"t_ms": 40, "kind": "review_flag", "reason": "token_cap", "final": false}
{"t_ms": 50, "kind": "tool_result", "name": "identify", "result": {}, "ms": 2}
{"t_ms": 60, "kind": "call_end", "reason": "the caller hung up", "tokens": {"input_tokens": 4, "output_tokens": 9}, "cost_usd": 0.0001}
"#;

fn key() -> SealKey {
    SealKey::from_bytes(&[9u8; 32]).unwrap()
}

/// The test key (DATABASE.md §7.4): seals the synthetic schema.
fn test_key() -> SealKey {
    SealKey::from_bytes(&[5u8; 32]).unwrap()
}

fn keys() -> StoreKeys {
    StoreKeys { hospital: key(), test: test_key() }
}

#[tokio::test]
async fn a_record_is_filed_once_sealed_and_again_when_it_changes() {
    let Ok(url) = std::env::var("DEETS_DATABASE_URL") else {
        eprintln!("skipped: DEETS_DATABASE_URL is not set");
        return;
    };
    let mut admin = PgConnection::connect(&url).await.unwrap();
    let name = format!("deets_store_test_{}", std::process::id());
    sqlx::query(sql(format!("DROP DATABASE IF EXISTS {name}"))).execute(&mut admin).await.unwrap();
    sqlx::query(sql(format!("CREATE DATABASE {name}"))).execute(&mut admin).await.unwrap();
    let base = url.rsplit_once('/').unwrap().0;
    let test_url = format!("{base}/{name}");

    let store = Store::connect(&test_url, keys()).await.unwrap();
    assert!(store.file_record("typed_a", RECORD.as_bytes()).await.unwrap(), "filed");
    assert!(!store.file_record("typed_a", RECORD.as_bytes()).await.unwrap(), "unchanged: skipped");
    // The same record with Windows line ends (as Python writes it there) is the same record.
    let crlf = RECORD.replace('\n', "\r\n");
    assert!(!store.file_record("typed_a", crlf.as_bytes()).await.unwrap(), "\\r\\n is the same record");

    let (outcome, turns): (String, i32) = sqlx::query_as("SELECT outcome, turns FROM calls WHERE id = 'typed_a'")
        .fetch_one(store.synthetic().pool())
        .await
        .unwrap();
    assert_eq!((outcome.as_str(), turns), ("abandoned", 1));
    let lines: i64 = sqlx::query_scalar("SELECT count(*) FROM transcript_lines").fetch_one(store.synthetic().pool()).await.unwrap();
    let events: i64 = sqlx::query_scalar("SELECT count(*) FROM call_events").fetch_one(store.synthetic().pool()).await.unwrap();
    let flags: i64 = sqlx::query_scalar("SELECT count(*) FROM staff_flags").fetch_one(store.synthetic().pool()).await.unwrap();
    let reviews: i64 = sqlx::query_scalar("SELECT count(*) FROM review_flags").fetch_one(store.synthetic().pool()).await.unwrap();
    assert_eq!((lines, events, flags, reviews), (2, 7, 1, 1));

    // The caller's words are sealed: no plaintext in the row, and they open only in their place.
    let words: Vec<u8> = sqlx::query_scalar("SELECT words FROM transcript_lines WHERE call_id = 'typed_a' AND seq = 2")
        .fetch_one(store.synthetic().pool())
        .await
        .unwrap();
    assert!(!words.windows(6).any(|w| w == b"Jordan"));
    let at = Place { table: "transcript_lines", row: "typed_a/2", column: "words" };
    assert_eq!(&*open_text(&AwsLc, &test_key(), &at, &words).unwrap(), "Jordan Ellis, March 12, 1984");

    // A changed record is filed again, whole: no rows left over from the old one.
    let shorter: String = RECORD.lines().take(3).collect::<Vec<_>>().join("\n");
    assert!(store.file_record("typed_a", shorter.as_bytes()).await.unwrap());
    let events: i64 = sqlx::query_scalar("SELECT count(*) FROM call_events").fetch_one(store.synthetic().pool()).await.unwrap();
    assert_eq!(events, 3);

    // Annotations: each line once, by its hash.
    let notes = "{\"kind\": \"label\", \"call\": \"typed_a\", \"text\": \"a label\"}\n{\"kind\": \"keep\", \"call\": \"typed_a\", \"value\": true}\n";
    assert_eq!(store.file_annotations(notes.as_bytes()).await.unwrap(), 2);
    assert_eq!(store.file_annotations(notes.as_bytes()).await.unwrap(), 0);

    // The settings store (CONSOLE.md §7.3): each save is a new version, numbered within its day,
    // and the active one; the history is insert-only.
    let pool = store.pool();
    assert_eq!(settings::get(pool, "voice", "settings_voice", 0).await.unwrap(), None, "no version yet");
    let set = |speed: serde_json::Value| -> Vec<(&'static str, serde_json::Value)> {
        settings::VOICE
            .iter()
            .map(|s| {
                let v = match s.key {
                    "voice" => json!({"name": "Skylar", "id": "db6b0ed5-d5d3-463d-ae85-518a07d3c2b4"}),
                    "speed" => speed.clone(),
                    "normalization" => json!("auto"),
                    "pronunciations" | "keyterms" => json!([]),
                    "eager_end_of_turn" => json!(false),
                    "barge_in" => json!(true),
                    "idle_s" => json!(60),
                    "max_s" => json!(360),
                    "plan_credits" => json!(20000),
                    "offset" => json!(270),
                    "cycle_day" => json!(12),
                    _ => json!(null),
                };
                (s.key, v)
            })
            .collect()
    };
    // The Default version first: number 0, active because nothing else is; only one.
    let default = settings::save(pool, "voice", "settings_voice", &set(json!(null)), "dev", true).await.unwrap();
    assert_eq!((default.seq, default.active), (0, true));
    assert_eq!(
        settings::save(pool, "voice", "settings_voice", &set(json!(null)), "dev", true).await,
        Err(settings::SaveError::DefaultExists)
    );
    let first = settings::save(pool, "voice", "settings_voice", &set(json!(null)), "dev", false).await.unwrap();
    let second = settings::save(pool, "voice", "settings_voice", &set(json!(1.2)), "dev", false).await.unwrap();
    assert_eq!((first.seq, second.seq, second.active), (1, 2, true));
    assert_eq!(first.day, second.day);
    let versions = settings::versions(pool, "voice").await.unwrap();
    assert_eq!(
        versions.iter().map(|v| (v.seq, v.active)).collect::<Vec<_>>(),
        vec![(2, true), (1, false), (0, false)],
        "newest first, the Default last"
    );
    let (active, values) = settings::get(pool, "voice", "settings_voice", 0).await.unwrap().unwrap();
    assert_eq!(active.id, second.id);
    assert!(values.contains(&("speed".to_string(), "1.2".to_string())));
    assert_eq!(values.len(), settings::VOICE.len());
    let (old, values) = settings::get(pool, "voice", "settings_voice", first.id).await.unwrap().unwrap();
    assert!(!old.active && values.contains(&("speed".to_string(), "null".to_string())));
    for sql in [
        "UPDATE settings_voice SET value = '1.5' WHERE key = 'speed'",
        "DELETE FROM setting_activations",
        "UPDATE setting_versions SET seq = 9",
        "TRUNCATE settings_voice",
    ] {
        assert!(sqlx::query(sql).execute(pool).await.is_err(), "insert-only: {sql}");
    }

    store.close().await;
    sqlx::query(sql(format!("DROP DATABASE {name}"))).execute(&mut admin).await.unwrap();
}

/// The People list (users.rs; docs/ops/DEMO.md §3.5): the owner from the first seed, an add by
/// the owner, the sealed columns, last seen, the owner guard, a removal that keeps the row, and a
/// return on the same row.
#[tokio::test]
async fn the_people_list_seals_guards_the_owner_and_keeps_removed_rows() {
    use deetshealth_ehr::users::{self, UsersError};
    let Ok(url) = std::env::var("DEETS_DATABASE_URL") else {
        eprintln!("skipped: DEETS_DATABASE_URL is not set");
        return;
    };
    let mut admin = PgConnection::connect(&url).await.unwrap();
    let name = format!("deets_users_test_{}", std::process::id());
    sqlx::query(sql(format!("DROP DATABASE IF EXISTS {name}"))).execute(&mut admin).await.unwrap();
    sqlx::query(sql(format!("CREATE DATABASE {name}"))).execute(&mut admin).await.unwrap();
    let base = url.rsplit_once('/').unwrap().0;
    let store = Store::connect(&format!("{base}/{name}"), keys()).await.unwrap();
    let pool = store.pool();

    // The first seed with one email: the owner. A second seed adds others, never a second owner.
    assert_eq!(users::seed(&store, &["Owner@Example.com".into()]).await.unwrap(), 1);
    assert_eq!(users::seed(&store, &["owner@example.com".into(), "dash@example.com".into()]).await.unwrap(), 1);
    let people = users::list(&store).await.unwrap();
    assert_eq!(people.len(), 2);
    assert!(people[0].owner && people[0].email.as_str() == "owner@example.com");
    assert!(!people[1].owner && people[1].name.as_str() == "dash@example.com");

    // An add from the owner's PC (`by` empty) names the owner as the adder.
    let id = users::add(&store, "Priya@Example.com", " Priya  Shah ", "").await.unwrap();
    assert_eq!(users::add(&store, "priya@example.com", "Again", "").await, Err(UsersError::Exists));
    let priya = users::list(&store).await.unwrap().into_iter().find(|u| u.id == id).unwrap();
    assert_eq!((priya.name.as_str(), priya.added_by.as_str()), ("Priya Shah", "owner@example.com"));
    assert!(priya.last_seen_at.is_none());

    // Sealed: neither the email nor the name is in the row as plain text.
    let row: (Vec<u8>, Vec<u8>) = sqlx::query_as("SELECT email, name FROM users WHERE id = $1::uuid").bind(&id).fetch_one(pool).await.unwrap();
    for bytes in [&row.0, &row.1] {
        let text = String::from_utf8_lossy(bytes);
        assert!(!text.contains("priya") && !text.contains("Priya"));
    }

    // Seen; then removed (the row stays); then not let in; then back on the same row.
    assert!(users::admit(&store, "PRIYA@example.com").await.unwrap());
    assert!(users::list(&store).await.unwrap().iter().any(|u| u.id == id && u.last_seen_at.is_some()));
    let owner_id = users::list(&store).await.unwrap()[0].id.clone();
    assert_eq!(users::remove(&store, &owner_id).await.err(), Some(UsersError::Owner));
    assert_eq!(users::remove(&store, &id).await.unwrap().as_str(), "priya@example.com");
    assert_eq!(users::remove(&store, &id).await.err(), Some(UsersError::NotFound));
    assert!(!users::admit(&store, "priya@example.com").await.unwrap());
    let kept: i64 = sqlx::query_scalar("SELECT count(*) FROM users WHERE removed_at IS NOT NULL").fetch_one(pool).await.unwrap();
    assert_eq!(kept, 1);
    // A seed never brings a removed person back.
    assert_eq!(users::seed(&store, &["priya@example.com".into()]).await.unwrap(), 0);
    assert!(!users::admit(&store, "priya@example.com").await.unwrap());
    let again = users::add(&store, "priya@example.com", "Priya S.", "dash@example.com").await.unwrap();
    assert_eq!(again, id);
    let back = users::list(&store).await.unwrap().into_iter().find(|u| u.id == id).unwrap();
    assert_eq!((back.name.as_str(), back.added_by.as_str()), ("Priya S.", "dash@example.com"));

    store.close().await;
    sqlx::query(sql(format!("DROP DATABASE {name}"))).execute(&mut admin).await.unwrap();
}

/// The associated data of formats 1 and 2, built by hand: the old domain, then the place, each
/// part with its length in front (core's seal.rs).
fn old_aad(table: &str, row: &str, column: &str) -> Vec<u8> {
    let mut out = Vec::new();
    for part in ["deetshealth.cache.v1", table, row, column] {
        out.extend_from_slice(&(part.len() as u32).to_le_bytes());
        out.extend_from_slice(part.as_bytes());
    }
    out
}

/// A value as a store sealed it before 2026-11-14: format 2, padded, under the old domain.
fn seal_format_2(with: &SealKey, table: &str, row: &str, column: &str, plain: &[u8]) -> Vec<u8> {
    use deetshealth_core::platform::Cipher;
    let mut padded = (plain.len() as u32).to_le_bytes().to_vec();
    padded.extend_from_slice(plain);
    padded.resize(deetshealth_core::seal::padded_len(4 + plain.len()), 0);
    let mut out = vec![2u8];
    out.extend(AwsLc.seal(with, &old_aad(table, row, column), &padded).unwrap());
    out
}

/// The format change of 2026-11-14 (core's seal.rs, format 3): a store sealed in format 2 is
/// sealed again at the next connect, nothing is lost, a value in the wrong place is left alone,
/// and the lists of sealed columns are the schemas'.
#[tokio::test]
async fn an_older_store_is_sealed_again_in_the_current_format() {
    use deetshealth_core::seal::{is_current, open};
    use deetshealth_ehr::store::{sealed_in, Resealed};
    use deetshealth_ehr::users;
    use sqlx::Row;
    let Ok(url) = std::env::var("DEETS_DATABASE_URL") else {
        eprintln!("skipped: DEETS_DATABASE_URL is not set");
        return;
    };
    let mut admin = PgConnection::connect(&url).await.unwrap();
    let name = format!("deets_reseal_test_{}", std::process::id());
    sqlx::query(sql(format!("DROP DATABASE IF EXISTS {name}"))).execute(&mut admin).await.unwrap();
    sqlx::query(sql(format!("CREATE DATABASE {name}"))).execute(&mut admin).await.unwrap();
    let base = url.rsplit_once('/').unwrap().0;
    let test_url = format!("{base}/{name}");
    let store = Store::connect(&test_url, keys()).await.unwrap();
    let pool = store.pool();

    // Each schema's list is the schema's: every `bytea` column is on it, but the keyed hash a
    // user is found by, which is not a sealed value.
    for schema_name in ["public", "synthetic", "real"] {
        let mut schema: Vec<(String, String)> = sqlx::query_as(
            "SELECT table_name::text, column_name::text FROM information_schema.columns
             WHERE table_schema = $1 AND data_type = 'bytea' AND table_name NOT LIKE '\\_sqlx%'
               AND (table_name, column_name) <> ('users', 'email_hash')",
        )
        .bind(schema_name)
        .fetch_all(pool)
        .await
        .unwrap();
        let mut listed: Vec<(String, String)> = sealed_in(schema_name)
            .iter()
            .flat_map(|t| t.columns.iter().map(|c| (t.table.to_string(), c.to_string())))
            .collect();
        schema.sort();
        listed.sort();
        assert_eq!(schema, listed, "store.rs names every sealed column of {schema_name}");
    }

    // A value in every sealed column.
    store.file_record("typed_a", RECORD.as_bytes()).await.unwrap();
    let notes = "{\"kind\": \"label\", \"call\": \"typed_a\", \"text\": \"a label\"}\n{\"kind\": \"keep\", \"call\": \"typed_a\", \"value\": true}\n";
    store.file_annotations(notes.as_bytes()).await.unwrap();
    users::seed(&store, &["owner@example.com".into()]).await.unwrap();
    let id = users::add(&store, "mary@example.com", "Mary Lee", "").await.unwrap();
    let card = users::Profile { first_name: "Mary", last_name: "Lee", phone: "(555) 234-5678", email: "mary@example.com" };
    users::update(&store, &id, &card).await.unwrap();
    let webp = b"RIFF\x10\0\0\0WEBPVP8 face-bytes".to_vec();
    users::set_photo(&store, &id, &webp).await.unwrap();
    assert_eq!(store.reseal().await.unwrap(), Resealed::default(), "a current store: nothing to do");

    // Turn it into a store from before the change: every value in format 2, each under its
    // schema's key. The owner login names the tables with their schema.
    let mut old_values = 0;
    for (schema_name, with) in [("public", key()), ("synthetic", test_key())] {
        for t in sealed_in(schema_name) {
            let table = format!("{schema_name}.{}", t.table);
            let rows = sqlx::query(sql(format!("SELECT {} AS k, {} FROM {table}", t.row, t.columns.join(", ")))).fetch_all(pool).await.unwrap();
            for r in rows {
                let k: String = r.get("k");
                for column in t.columns {
                    let Some(bytes) = r.get::<Option<Vec<u8>>, _>(*column) else { continue };
                    assert!(is_current(&bytes));
                    let plain = open(&AwsLc, &with, &Place { table: t.table, row: &k, column }, &bytes).unwrap();
                    sqlx::query(sql(format!("UPDATE {table} SET {column} = $1 WHERE {} = $2", t.row)))
                        .bind(seal_format_2(&with, t.table, &k, column, &plain))
                        .bind(&k)
                        .execute(pool)
                        .await
                        .unwrap();
                    old_values += 1;
                }
            }
        }
    }
    // 3 lines of words (2 transcript, 1 label), 7 events, a flag's 2, the owner's 2, Mary's 5, 1 photo.
    assert_eq!(old_values, 2 + 7 + 2 + 1 + 2 + 5 + 1);
    // An old store still reads, before it is sealed again.
    assert_eq!(users::photo(&store, &id).await.unwrap().unwrap().as_slice(), webp.as_slice());

    // One value that belongs to another row: it must not open, and must not be lost.
    let stray = seal_format_2(&test_key(), "transcript_lines", "typed_a/9", "words", b"another row's words");
    sqlx::query("UPDATE synthetic.transcript_lines SET words = $1 WHERE call_id = 'typed_a' AND seq = 1").bind(&stray).execute(pool).await.unwrap();

    // The next connect seals it all again.
    store.close().await;
    let store = Store::connect(&test_url, keys()).await.unwrap();
    let pool = store.pool();
    let mut current = 0;
    for schema_name in ["public", "synthetic"] {
        for t in sealed_in(schema_name) {
            for column in t.columns {
                let values: Vec<Option<Vec<u8>>> =
                    sqlx::query_scalar(sql(format!("SELECT {column} FROM {schema_name}.{}", t.table))).fetch_all(pool).await.unwrap();
                current += values.iter().flatten().filter(|b| is_current(b)).count();
            }
        }
    }
    assert_eq!(current, old_values - 1, "every value but the stray one is current");
    let kept: Vec<u8> =
        sqlx::query_scalar("SELECT words FROM synthetic.transcript_lines WHERE call_id = 'typed_a' AND seq = 1").fetch_one(pool).await.unwrap();
    assert_eq!(kept, stray, "a value that does not open keeps its bytes");
    assert_eq!(store.reseal().await.unwrap(), Resealed { values: 0, unopened: 1 }, "again: only the stray value is left");

    // Nothing lost: the words, the person and the picture read back.
    let words: Vec<u8> =
        sqlx::query_scalar("SELECT words FROM synthetic.transcript_lines WHERE call_id = 'typed_a' AND seq = 2").fetch_one(pool).await.unwrap();
    let at = Place { table: "transcript_lines", row: "typed_a/2", column: "words" };
    assert_eq!(&*open_text(&AwsLc, &test_key(), &at, &words).unwrap(), "Jordan Ellis, March 12, 1984");
    let mary = users::list(&store).await.unwrap().into_iter().find(|u| u.id == id).unwrap();
    assert_eq!((mary.email.as_str(), mary.name.as_str(), mary.phone.as_str()), ("mary@example.com", "Mary Lee", "+15552345678"));
    assert_eq!(users::photo(&store, &id).await.unwrap().unwrap().as_slice(), webp.as_slice());

    store.close().await;
    sqlx::query(sql(format!("DROP DATABASE {name}"))).execute(&mut admin).await.unwrap();
}

/// Synthetic and real kept apart (DATABASE.md §7): the class pool sees its schema only, the
/// checks hold, the two schemas have the same columns, and the rows sealed under the hospital
/// key before migration 0010 move to the test key once.
#[tokio::test]
async fn synthetic_and_real_rows_are_kept_apart() {
    use deetshealth_core::seal::seal;
    let Ok(url) = std::env::var("DEETS_DATABASE_URL") else {
        eprintln!("skipped: DEETS_DATABASE_URL is not set");
        return;
    };
    let mut admin = PgConnection::connect(&url).await.unwrap();
    let name = format!("deets_class_test_{}", std::process::id());
    sqlx::query(sql(format!("DROP DATABASE IF EXISTS {name}"))).execute(&mut admin).await.unwrap();
    sqlx::query(sql(format!("CREATE DATABASE {name}"))).execute(&mut admin).await.unwrap();
    let base = url.rsplit_once('/').unwrap().0;
    let test_url = format!("{base}/{name}");
    let store = Store::connect(&test_url, keys()).await.unwrap();
    let synthetic = store.synthetic().pool();

    // The synthetic pool cannot name a real table, nor the people list.
    for sql in ["SELECT count(*) FROM real.calls", "SELECT count(*) FROM public.users", "SELECT count(*) FROM real.annotations"] {
        assert!(sqlx::query(sql).execute(synthetic).await.is_err(), "the synthetic role must not read: {sql}");
    }
    assert!(store.class(DataClass::Real).is_none(), "a dev machine has no real store");
    let real = RECORD.replace("\"source\": \"typed\"", "\"source\": \"live\", \"data_class\": \"real\"");
    assert!(store.file_record("live_a", real.as_bytes()).await.is_err(), "no real store: refused");

    // The id starts with its origin; a real row is only a live one; a synthetic row stays synthetic.
    assert!(store.file_record("test_a", RECORD.as_bytes()).await.is_err(), "a typed record under a test id");
    assert!(store.file_record("typed_a", RECORD.as_bytes()).await.unwrap());
    let owner = store.pool();
    for sql in [
        "INSERT INTO real.calls (id, schema_version, origin, outcome, record_sha256) VALUES ('test_x', 1, 'test', 'ended', 'x')",
        "INSERT INTO synthetic.calls (id, schema_version, origin, outcome, record_sha256, data_class) VALUES ('live_x', 1, 'live', 'ended', 'x', 'real')",
        "INSERT INTO synthetic.calls (id, schema_version, origin, outcome, record_sha256) VALUES ('kept_x', 1, 'test', 'ended', 'x')",
        "INSERT INTO synthetic.calls (id, schema_version, origin, outcome, record_sha256) VALUES ('demo_x', 1, 'demo', 'ended', 'x')",
    ] {
        assert!(sqlx::query(sql).execute(owner).await.is_err(), "the checks must refuse: {sql}");
    }
    sqlx::query("INSERT INTO real.calls (id, schema_version, origin, outcome, record_sha256) VALUES ('live_x', 1, 'live', 'ended', 'x')")
        .execute(owner)
        .await
        .unwrap();
    let by_class: Vec<(String, String)> = sqlx::query_as("SELECT id, data_class FROM public.calls_by_class ORDER BY id").fetch_all(owner).await.unwrap();
    assert_eq!(by_class, vec![("live_x".into(), "real".into()), ("typed_a".into(), "synthetic".into())]);

    // One template: the two schemas' call tables have the same columns.
    let columns = |schema: &'static str| {
        sqlx::query_as::<_, (String, String, String)>(
            "SELECT table_name::text, column_name::text, data_type::text FROM information_schema.columns
             WHERE table_schema = $1 AND table_name NOT LIKE 'test\\_%' ORDER BY 1, 2",
        )
        .bind(schema)
        .fetch_all(owner)
    };
    assert_eq!(columns("synthetic").await.unwrap(), columns("real").await.unwrap(), "a change to a call table is made in both schemas");

    // The move to the test key: a store from before 0010 had its call rows under the hospital key.
    let row = "typed_a/2";
    let old = seal(&AwsLc, &key(), &Place { table: "transcript_lines", row, column: "words" }, b"under the old key").unwrap();
    sqlx::query("UPDATE synthetic.transcript_lines SET words = $1 WHERE call_id = 'typed_a' AND seq = 2").bind(&old).execute(owner).await.unwrap();
    sqlx::query("DELETE FROM store_meta").execute(owner).await.unwrap();
    store.close().await;
    let store = Store::connect(&test_url, keys()).await.unwrap();
    let moved: Vec<u8> =
        sqlx::query_scalar("SELECT words FROM transcript_lines WHERE call_id = 'typed_a' AND seq = 2").fetch_one(store.synthetic().pool()).await.unwrap();
    let at = Place { table: "transcript_lines", row, column: "words" };
    assert_eq!(&*open_text(&AwsLc, &test_key(), &at, &moved).unwrap(), "under the old key", "moved to the test key");
    assert!(open_text(&AwsLc, &key(), &at, &moved).is_err(), "the hospital key no longer opens it");
    let marked: i64 = sqlx::query_scalar("SELECT count(*) FROM store_meta WHERE key = 'key.synthetic'").fetch_one(store.pool()).await.unwrap();
    assert_eq!(marked, 1, "the move is done once");

    store.close().await;
    sqlx::query(sql(format!("DROP DATABASE {name}"))).execute(&mut admin).await.unwrap();
}

/// The kept scripts' runs (DATABASE.md §8): a run files its draws; a passing replay draw with the
/// same shape as the last filed draw files no record (option C); a changed or failed draw does;
/// the failure line is sealed; the retention pass keeps the newest words versions and stars.
#[tokio::test]
async fn a_test_run_files_its_draws_and_reuses_a_same_shape_record() {
    use deetshealth_ehr::store::{Annotation, TestResult, TestRun};
    use zeroize::Zeroizing;
    let Ok(url) = std::env::var("DEETS_DATABASE_URL") else {
        eprintln!("skipped: DEETS_DATABASE_URL is not set");
        return;
    };
    let mut admin = PgConnection::connect(&url).await.unwrap();
    let name = format!("deets_runs_test_{}", std::process::id());
    sqlx::query(sql(format!("DROP DATABASE IF EXISTS {name}"))).execute(&mut admin).await.unwrap();
    sqlx::query(sql(format!("CREATE DATABASE {name}"))).execute(&mut admin).await.unwrap();
    let base = url.rsplit_once('/').unwrap().0;
    let store = Store::connect(&format!("{base}/{name}"), keys()).await.unwrap();
    let synthetic = store.synthetic();
    let kept = RECORD.replace("\"source\": \"typed\"", "\"source\": \"kept\"");
    let words = |n: u8| format!("{n:02x}").repeat(32);
    let draw = |id: &str, shape: &str, passed: bool| TestResult {
        script: "book_a_visit".into(),
        draw: 1,
        mode: "replay".into(),
        passed,
        test: "test_book_a_visit".into(),
        failure: (!passed).then(|| Zeroizing::new("AssertionError: the visit should be booked".to_string())),
        call_id: id.into(),
        shape_sha256: shape.into(),
        duration_ms: 812.5,
        tool_ms: "{\"book\": 3.1}".into(),
        tags: "{\"safety\": \"no\", \"area\": \"book\"}".into(),
        reply_keys: vec!["0d2fb76ec51e2f9b373536be8e443316".into()],
        ..Default::default()
    };
    let run = |w: u8, results: Vec<TestResult>| TestRun {
        started: format!("2026-11-15T20:{w:02}:00Z"),
        run_by: "dev".into(),
        words_sha256: words(w),
        tests_passed: true,
        results,
        ..Default::default()
    };

    // The first run: no earlier draw, so the store needs the record.
    let first = synthetic.file_test_run(&run(1, vec![draw("kept_1", &"a".repeat(64), true)])).await.unwrap();
    assert_eq!(first.needed, vec!["kept_1".to_string()]);
    synthetic.file_record("kept_1", kept.as_bytes()).await.unwrap();
    // The same shape again: reused, no record.
    let second = synthetic.file_test_run(&run(1, vec![draw("kept_2", &"a".repeat(64), true)])).await.unwrap();
    assert!(second.needed.is_empty(), "a passing draw of the same shape files no record");
    let (call, reused): (Option<String>, bool) = sqlx::query_as("SELECT call_id, reused FROM test_results WHERE run_id = $1")
        .bind(second.run_id)
        .fetch_one(synthetic.pool())
        .await
        .unwrap();
    assert_eq!((call.as_deref(), reused), (Some("kept_1"), true));
    // A changed shape, and a failure: both need their records; the failure line is sealed.
    let third = synthetic.file_test_run(&run(1, vec![draw("kept_3", &"b".repeat(64), true)])).await.unwrap();
    assert_eq!(third.needed, vec!["kept_3".to_string()]);
    let fourth = synthetic.file_test_run(&run(1, vec![draw("kept_4", &"a".repeat(64), false)])).await.unwrap();
    assert_eq!(fourth.needed, vec!["kept_4".to_string()]);
    let failure: Vec<u8> = sqlx::query_scalar("SELECT failure FROM test_results WHERE run_id = $1").bind(fourth.run_id).fetch_one(synthetic.pool()).await.unwrap();
    assert!(!String::from_utf8_lossy(&failure).contains("booked"), "the failure line is sealed");
    let (scripts, failed): (i32, i32) = sqlx::query_as("SELECT scripts, failed FROM test_runs WHERE id = $1").bind(fourth.run_id).fetch_one(synthetic.pool()).await.unwrap();
    assert_eq!((scripts, failed), (1, 1));
    // A draw's id must be a kept one.
    assert!(synthetic.file_test_run(&run(1, vec![draw("test_9", &"c".repeat(64), true)])).await.is_err());

    // Retention: three newer words versions push kept_1 out, unless it is starred.
    let star = Annotation { call_id: "kept_1".into(), kind: "keep".into(), at: "2026-11-15T20:05:00".into(), keep: Some(true), ..Default::default() };
    assert!(synthetic.add_annotation(&star).await.unwrap());
    for w in 2..=4 {
        synthetic.file_test_run(&run(w, vec![])).await.unwrap();
    }
    assert!(synthetic.has_call("kept_1").await.unwrap(), "a starred kept record stays");
    let unstar = Annotation { at: "2026-11-15T20:06:00".into(), keep: Some(false), ..star.clone() };
    synthetic.add_annotation(&unstar).await.unwrap();
    let pruned = synthetic.file_test_run(&run(5, vec![])).await.unwrap().pruned;
    assert_eq!(pruned, 1, "kept_1 goes once unstarred; kept_3 was never filed");
    assert!(!synthetic.has_call("kept_1").await.unwrap());
    let runs: i64 = sqlx::query_scalar("SELECT count(*) FROM test_runs").fetch_one(synthetic.pool()).await.unwrap();
    assert_eq!(runs, 8, "the runs themselves are kept (the refused one rolled back)");

    store.close().await;
    sqlx::query(sql(format!("DROP DATABASE {name}"))).execute(&mut admin).await.unwrap();
}

/// Annotations from the console (DATABASE.md §9): filed once in the call's class, sealed, read
/// back opened; a call the store does not hold is refused.
#[tokio::test]
async fn an_annotation_is_filed_once_and_read_back() {
    use deetshealth_ehr::store::Annotation;
    use zeroize::Zeroizing;
    let Ok(url) = std::env::var("DEETS_DATABASE_URL") else {
        eprintln!("skipped: DEETS_DATABASE_URL is not set");
        return;
    };
    let mut admin = PgConnection::connect(&url).await.unwrap();
    let name = format!("deets_notes_test_{}", std::process::id());
    sqlx::query(sql(format!("DROP DATABASE IF EXISTS {name}"))).execute(&mut admin).await.unwrap();
    sqlx::query(sql(format!("CREATE DATABASE {name}"))).execute(&mut admin).await.unwrap();
    let base = url.rsplit_once('/').unwrap().0;
    let store = Store::connect(&format!("{base}/{name}"), keys()).await.unwrap();
    store.file_record("typed_a", RECORD.as_bytes()).await.unwrap();
    let synthetic = store.synthetic();
    let rating = Annotation {
        call_id: "typed_a".into(),
        kind: "rating".into(),
        at: "2026-11-15T20:00:00".into(),
        grader_kind: Some("human".into()),
        grader: Some("owner".into()),
        dimension: Some("phone_style".into()),
        verdict: Some("pass".into()),
        score: Some(4),
        rubric_version: Some("2026-11-12".into()),
        words: Some(Zeroizing::new("short and clear".into())),
        ..Default::default()
    };
    assert!(synthetic.add_annotation(&rating).await.unwrap());
    assert!(!synthetic.add_annotation(&rating).await.unwrap(), "the same annotation is filed once");
    let sealed: Vec<u8> = sqlx::query_scalar("SELECT words FROM annotations").fetch_one(synthetic.pool()).await.unwrap();
    assert!(!String::from_utf8_lossy(&sealed).contains("clear"));
    let back = synthetic.list_annotations().await.unwrap();
    assert_eq!(back.len(), 1);
    assert_eq!(back[0], rating, "read back as it went in");
    let missing = Annotation { call_id: "typed_zzz".into(), ..rating.clone() };
    assert!(synthetic.add_annotation(&missing).await.is_err(), "no such call");
    assert!(store.class_of_call("typed_a").await.unwrap().is_some());
    assert!(store.class_of_call("typed_zzz").await.unwrap().is_none());

    store.close().await;
    sqlx::query(sql(format!("DROP DATABASE {name}"))).execute(&mut admin).await.unwrap();
}

/// A person's card (users.rs › update, the picture; migration 0008): the old name split, a save
/// sealed, the email moved to its new hash, a taken email refused, the phone's mark, the owner's
/// last seen, and the picture sealed under the person's id.
#[tokio::test]
async fn a_card_saves_sealed_moves_its_email_and_keeps_a_picture() {
    use deetshealth_ehr::users::{self, Profile, UsersError};
    let Ok(url) = std::env::var("DEETS_DATABASE_URL") else {
        eprintln!("skipped: DEETS_DATABASE_URL is not set");
        return;
    };
    let mut admin = PgConnection::connect(&url).await.unwrap();
    let name = format!("deets_cards_test_{}", std::process::id());
    sqlx::query(sql(format!("DROP DATABASE IF EXISTS {name}"))).execute(&mut admin).await.unwrap();
    sqlx::query(sql(format!("CREATE DATABASE {name}"))).execute(&mut admin).await.unwrap();
    let base = url.rsplit_once('/').unwrap().0;
    let store = Store::connect(&format!("{base}/{name}"), keys()).await.unwrap();
    let pool = store.pool();

    users::seed(&store, &["owner@example.com".into()]).await.unwrap();
    let id = users::add(&store, "mary@example.com", "Mary Ann Lee", "").await.unwrap();
    let gone = users::add(&store, "gone@example.com", "Gone", "").await.unwrap();
    users::remove(&store, &gone).await.unwrap();
    let find = |list: Vec<users::User>, id: &str| list.into_iter().find(|u| u.id == id).unwrap();

    // Before a save: the name split at its last space; the owner's email-name is no name.
    let people = users::list(&store).await.unwrap();
    assert_eq!((people[0].first_name.as_str(), people[0].last_name.as_str()), ("", ""));
    let mary = find(people, &id);
    assert_eq!((mary.first_name.as_str(), mary.last_name.as_str()), ("Mary Ann", "Lee"));
    assert!(mary.phone.is_empty() && mary.photo_at.is_none() && mary.source == "access");

    // A save: the parts as typed, the phone as E.164, not verified; every column sealed.
    let card = |email: &'static str| Profile { first_name: "Mary", last_name: "de la Cruz", phone: "(555) 234-5678", email };
    let saved = users::update(&store, &id, &card("mary@example.com")).await.unwrap();
    assert!(!saved.email_changed);
    let mary = find(users::list(&store).await.unwrap(), &id);
    assert_eq!((mary.first_name.as_str(), mary.last_name.as_str(), mary.name.as_str()), ("Mary", "de la Cruz", "Mary de la Cruz"));
    assert_eq!((mary.phone.as_str(), mary.phone_verified), ("+15552345678", false));
    let row: (Vec<u8>, Vec<u8>, Vec<u8>) =
        sqlx::query_as("SELECT first_name, last_name, phone FROM users WHERE id = $1::uuid").bind(&id).fetch_one(pool).await.unwrap();
    for bytes in [&row.0, &row.1, &row.2] {
        let text = String::from_utf8_lossy(bytes);
        assert!(!text.contains("Mary") && !text.contains("Cruz") && !text.contains("5552345678"));
    }

    // A verified number keeps its mark on a save that keeps the number, and loses it on a change.
    sqlx::query("UPDATE users SET phone_verified = true WHERE id = $1::uuid").bind(&id).execute(pool).await.unwrap();
    users::update(&store, &id, &card("mary@example.com")).await.unwrap();
    assert!(find(users::list(&store).await.unwrap(), &id).phone_verified);
    let other = Profile { phone: "555-234-0000", ..card("mary@example.com") };
    users::update(&store, &id, &other).await.unwrap();
    assert!(!find(users::list(&store).await.unwrap(), &id).phone_verified);

    // The email: someone else's, or a removed person's, is refused; a new one moves the row.
    assert_eq!(users::update(&store, &id, &card("owner@example.com")).await.err(), Some(UsersError::Exists));
    assert_eq!(users::update(&store, &id, &card("gone@example.com")).await.err(), Some(UsersError::Taken));
    let moved = users::update(&store, &id, &card("Mary.Lee@Example.com")).await.unwrap();
    assert!(moved.email_changed && moved.old_email.as_str() == "mary@example.com");
    assert!(!users::admit(&store, "mary@example.com").await.unwrap());
    assert!(users::admit(&store, "mary.lee@example.com").await.unwrap());
    let mary = find(users::list(&store).await.unwrap(), &id);
    assert_eq!((mary.email.as_str(), mary.first_name.as_str(), mary.phone.as_str()), ("mary.lee@example.com", "Mary", "+15552345678"));

    // A saved email cannot be cleared; bad fields name the field.
    assert!(matches!(users::update(&store, &id, &card("")).await, Err(UsersError::Bad(_))));
    let bad_phone = Profile { phone: "555-1234", ..card("mary.lee@example.com") };
    assert!(matches!(users::update(&store, &id, &bad_phone).await, Err(UsersError::Bad(_))));
    assert_eq!(users::update(&store, &gone, &card("x@example.com")).await.err(), Some(UsersError::NotFound));

    // "Added by" is the adder as "First L.", read from their row: the owner's email until the
    // owner's card has a name, then the name.
    assert_eq!(find(users::list(&store).await.unwrap(), &id).added_by.as_str(), "owner@example.com");
    let owner_id = users::list(&store).await.unwrap()[0].id.clone();
    let olive = Profile { first_name: "Olive", last_name: "Ng", phone: "", email: "owner@example.com" };
    users::update(&store, &owner_id, &olive).await.unwrap();
    assert_eq!(find(users::list(&store).await.unwrap(), &id).added_by.as_str(), "Olive N.");

    // The owner's PC stamps the owner's last seen.
    assert!(users::list(&store).await.unwrap()[0].last_seen_at.is_none());
    users::seen_owner(&store).await.unwrap();
    assert!(users::list(&store).await.unwrap()[0].last_seen_at.is_some());

    // A card to finish later (the owner, 2026-11-14): no email, or no name, but not neither.
    assert!(matches!(users::add(&store, " ", " ", "").await, Err(UsersError::Bad(_))));
    let later = users::add(&store, "", "Test Lee", "").await.unwrap();
    let other = users::add(&store, "", "Sam", "").await.unwrap(); // two stand-in keys never collide
    assert_ne!(later, other);
    let test = find(users::list(&store).await.unwrap(), &later);
    assert_eq!((test.email.as_str(), test.first_name.as_str(), test.last_name.as_str()), ("", "Test", "Lee"));
    // Saved again with no email: the row keeps its stand-in key; nobody can sign in with it.
    let no_email = Profile { first_name: "Test", last_name: "Lee", phone: "555-234-5678", email: "" };
    let kept = users::update(&store, &later, &no_email).await.unwrap();
    assert!(!kept.email_changed);
    assert!(!users::admit(&store, "").await.unwrap());
    // Its first email moves it to that email's hash: now on the list for sign-in.
    let first = users::update(&store, &later, &Profile { email: "test@example.com", ..no_email }).await.unwrap();
    assert!(first.email_changed && first.old_email.is_empty());
    assert!(users::admit(&store, "test@example.com").await.unwrap());
    assert_eq!(find(users::list(&store).await.unwrap(), &later).phone.as_str(), "+15552345678");
    // No name, an email: saved.
    let nameless = users::add(&store, "nameless@example.com", "", "").await.unwrap();
    let blank = Profile { first_name: " ", last_name: "", phone: "", email: "nameless@example.com" };
    users::update(&store, &nameless, &blank).await.unwrap();
    assert_eq!(users::remove(&store, &other).await.unwrap().as_str(), "");

    // The picture: a WebP only, sealed, back as it went in, and cleared.
    let webp = b"RIFF\x10\0\0\0WEBPVP8 face-bytes".to_vec();
    assert!(matches!(users::set_photo(&store, &id, b"not a picture").await, Err(UsersError::Bad(_))));
    users::set_photo(&store, &id, &webp).await.unwrap();
    assert_eq!(users::photo(&store, &id).await.unwrap().unwrap().as_slice(), webp.as_slice());
    let sealed: Vec<u8> = sqlx::query_scalar("SELECT photo FROM user_photos WHERE user_id = $1::uuid").bind(&id).fetch_one(pool).await.unwrap();
    assert!(!String::from_utf8_lossy(&sealed).contains("face-bytes"));
    assert!(find(users::list(&store).await.unwrap(), &id).photo_at.is_some());
    users::clear_photo(&store, &id).await.unwrap();
    assert!(users::photo(&store, &id).await.unwrap().is_none());
    assert_eq!(users::photo(&store, &gone).await.err(), Some(UsersError::NotFound));

    store.close().await;
    sqlx::query(sql(format!("DROP DATABASE {name}"))).execute(&mut admin).await.unwrap();
}
