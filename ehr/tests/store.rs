//! The call store against a real Postgres (DATABASE.md). Skipped unless `DEETS_DATABASE_URL` is
//! set (`npm run db:test` sets it). Each run makes its own database and drops it after, so the
//! dev data is never touched.

use deetshealth_core::seal::{open_text, Place};
use deetshealth_ehr::{
    seal_key::{AwsLc, SealKey},
    settings,
    store::Store,
};
use serde_json::json;
use sqlx::{Connection, PgConnection};

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

#[tokio::test]
async fn a_record_is_filed_once_sealed_and_again_when_it_changes() {
    let Ok(url) = std::env::var("DEETS_DATABASE_URL") else {
        eprintln!("skipped: DEETS_DATABASE_URL is not set");
        return;
    };
    let mut admin = PgConnection::connect(&url).await.unwrap();
    let name = format!("deets_store_test_{}", std::process::id());
    sqlx::query(&format!("DROP DATABASE IF EXISTS {name}")).execute(&mut admin).await.unwrap();
    sqlx::query(&format!("CREATE DATABASE {name}")).execute(&mut admin).await.unwrap();
    let base = url.rsplit_once('/').unwrap().0;
    let test_url = format!("{base}/{name}");

    let store = Store::connect(&test_url, key()).await.unwrap();
    assert!(store.file_record("call-a", RECORD.as_bytes()).await.unwrap(), "filed");
    assert!(!store.file_record("call-a", RECORD.as_bytes()).await.unwrap(), "unchanged: skipped");
    // The same record with Windows line ends (as Python writes it there) is the same record.
    let crlf = RECORD.replace('\n', "\r\n");
    assert!(!store.file_record("call-a", crlf.as_bytes()).await.unwrap(), "\\r\\n is the same record");

    let (outcome, turns): (String, i32) = sqlx::query_as("SELECT outcome, turns FROM calls WHERE id = 'call-a'")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!((outcome.as_str(), turns), ("abandoned", 1));
    let lines: i64 = sqlx::query_scalar("SELECT count(*) FROM transcript_lines").fetch_one(store.pool()).await.unwrap();
    let events: i64 = sqlx::query_scalar("SELECT count(*) FROM call_events").fetch_one(store.pool()).await.unwrap();
    let flags: i64 = sqlx::query_scalar("SELECT count(*) FROM staff_flags").fetch_one(store.pool()).await.unwrap();
    let reviews: i64 = sqlx::query_scalar("SELECT count(*) FROM review_flags").fetch_one(store.pool()).await.unwrap();
    assert_eq!((lines, events, flags, reviews), (2, 7, 1, 1));

    // The caller's words are sealed: no plaintext in the row, and they open only in their place.
    let words: Vec<u8> = sqlx::query_scalar("SELECT words FROM transcript_lines WHERE call_id = 'call-a' AND seq = 2")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert!(!words.windows(6).any(|w| w == b"Jordan"));
    let at = Place { table: "transcript_lines", row: "call-a/2", column: "words" };
    assert_eq!(&*open_text(&AwsLc, &key(), &at, &words).unwrap(), "Jordan Ellis, March 12, 1984");

    // A changed record is filed again, whole: no rows left over from the old one.
    let shorter: String = RECORD.lines().take(3).collect::<Vec<_>>().join("\n");
    assert!(store.file_record("call-a", shorter.as_bytes()).await.unwrap());
    let events: i64 = sqlx::query_scalar("SELECT count(*) FROM call_events").fetch_one(store.pool()).await.unwrap();
    assert_eq!(events, 3);

    // Annotations: each line once, by its hash.
    let notes = "{\"kind\": \"label\", \"call\": \"call-a\", \"text\": \"a label\"}\n{\"kind\": \"keep\", \"call\": \"call-a\", \"value\": true}\n";
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

    store.pool().close().await;
    sqlx::query(&format!("DROP DATABASE {name}")).execute(&mut admin).await.unwrap();
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
    sqlx::query(&format!("DROP DATABASE IF EXISTS {name}")).execute(&mut admin).await.unwrap();
    sqlx::query(&format!("CREATE DATABASE {name}")).execute(&mut admin).await.unwrap();
    let base = url.rsplit_once('/').unwrap().0;
    let store = Store::connect(&format!("{base}/{name}"), key()).await.unwrap();
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

    store.pool().close().await;
    sqlx::query(&format!("DROP DATABASE {name}")).execute(&mut admin).await.unwrap();
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
fn seal_format_2(table: &str, row: &str, column: &str, plain: &[u8]) -> Vec<u8> {
    use deetshealth_core::platform::Cipher;
    let mut padded = (plain.len() as u32).to_le_bytes().to_vec();
    padded.extend_from_slice(plain);
    padded.resize(deetshealth_core::seal::padded_len(4 + plain.len()), 0);
    let mut out = vec![2u8];
    out.extend(AwsLc.seal(&key(), &old_aad(table, row, column), &padded).unwrap());
    out
}

/// The format change of 2026-11-14 (core's seal.rs, format 3): a store sealed in format 2 is
/// sealed again at the next connect, nothing is lost, a value in the wrong place is left alone,
/// and the list of sealed columns is the schema's.
#[tokio::test]
async fn an_older_store_is_sealed_again_in_the_current_format() {
    use deetshealth_core::seal::{is_current, open};
    use deetshealth_ehr::store::{Resealed, SEALED};
    use deetshealth_ehr::users;
    use sqlx::Row;
    let Ok(url) = std::env::var("DEETS_DATABASE_URL") else {
        eprintln!("skipped: DEETS_DATABASE_URL is not set");
        return;
    };
    let mut admin = PgConnection::connect(&url).await.unwrap();
    let name = format!("deets_reseal_test_{}", std::process::id());
    sqlx::query(&format!("DROP DATABASE IF EXISTS {name}")).execute(&mut admin).await.unwrap();
    sqlx::query(&format!("CREATE DATABASE {name}")).execute(&mut admin).await.unwrap();
    let base = url.rsplit_once('/').unwrap().0;
    let test_url = format!("{base}/{name}");
    let store = Store::connect(&test_url, key()).await.unwrap();
    let pool = store.pool();

    // The list is the schema's: every `bytea` column is on it, but the keyed hash a user is
    // found by, which is not a sealed value.
    let mut schema: Vec<(String, String)> = sqlx::query_as(
        "SELECT table_name::text, column_name::text FROM information_schema.columns
         WHERE table_schema = 'public' AND data_type = 'bytea' AND table_name NOT LIKE '\\_sqlx%'
           AND (table_name, column_name) <> ('users', 'email_hash')",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    let mut listed: Vec<(String, String)> =
        SEALED.iter().flat_map(|t| t.columns.iter().map(|c| (t.table.to_string(), c.to_string()))).collect();
    schema.sort();
    listed.sort();
    assert_eq!(schema, listed, "store.rs › SEALED names every sealed column");

    // A value in every sealed column.
    store.file_record("call-a", RECORD.as_bytes()).await.unwrap();
    let notes = "{\"kind\": \"label\", \"call\": \"call-a\", \"text\": \"a label\"}\n{\"kind\": \"keep\", \"call\": \"call-a\", \"value\": true}\n";
    store.file_annotations(notes.as_bytes()).await.unwrap();
    users::seed(&store, &["owner@example.com".into()]).await.unwrap();
    let id = users::add(&store, "mary@example.com", "Mary Lee", "").await.unwrap();
    let card = users::Profile { first_name: "Mary", last_name: "Lee", phone: "(555) 234-5678", email: "mary@example.com" };
    users::update(&store, &id, &card).await.unwrap();
    let webp = b"RIFF\x10\0\0\0WEBPVP8 face-bytes".to_vec();
    users::set_photo(&store, &id, &webp).await.unwrap();
    assert_eq!(store.reseal().await.unwrap(), Resealed::default(), "a current store: nothing to do");

    // Turn it into a store from before the change: every value in format 2.
    let mut old_values = 0;
    for t in SEALED {
        let rows = sqlx::query(&format!("SELECT {} AS k, {} FROM {}", t.row, t.columns.join(", "), t.table)).fetch_all(pool).await.unwrap();
        for r in rows {
            let k: String = r.get("k");
            for column in t.columns {
                let Some(bytes) = r.get::<Option<Vec<u8>>, _>(*column) else { continue };
                assert!(is_current(&bytes));
                let plain = open(&AwsLc, &key(), &Place { table: t.table, row: &k, column }, &bytes).unwrap();
                sqlx::query(&format!("UPDATE {} SET {column} = $1 WHERE {} = $2", t.table, t.row))
                    .bind(seal_format_2(t.table, &k, column, &plain))
                    .bind(&k)
                    .execute(pool)
                    .await
                    .unwrap();
                old_values += 1;
            }
        }
    }
    // 3 lines of words (2 transcript, 1 label), 7 events, a flag's 2, the owner's 2, Mary's 5, 1 photo.
    assert_eq!(old_values, 2 + 7 + 2 + 1 + 2 + 5 + 1);
    // An old store still reads, before it is sealed again.
    assert_eq!(users::photo(&store, &id).await.unwrap().unwrap().as_slice(), webp.as_slice());

    // One value that belongs to another row: it must not open, and must not be lost.
    let stray = seal_format_2("transcript_lines", "call-a/9", "words", b"another row's words");
    sqlx::query("UPDATE transcript_lines SET words = $1 WHERE call_id = 'call-a' AND seq = 1").bind(&stray).execute(pool).await.unwrap();

    // The next connect seals it all again.
    store.pool().close().await;
    let store = Store::connect(&test_url, key()).await.unwrap();
    let pool = store.pool();
    let mut current = 0;
    for t in SEALED {
        for column in t.columns {
            let values: Vec<Option<Vec<u8>>> = sqlx::query_scalar(&format!("SELECT {column} FROM {}", t.table)).fetch_all(pool).await.unwrap();
            current += values.iter().flatten().filter(|b| is_current(b)).count();
        }
    }
    assert_eq!(current, old_values - 1, "every value but the stray one is current");
    let kept: Vec<u8> = sqlx::query_scalar("SELECT words FROM transcript_lines WHERE call_id = 'call-a' AND seq = 1").fetch_one(pool).await.unwrap();
    assert_eq!(kept, stray, "a value that does not open keeps its bytes");
    assert_eq!(store.reseal().await.unwrap(), Resealed { values: 0, unopened: 1 }, "again: only the stray value is left");

    // Nothing lost: the words, the person and the picture read back.
    let words: Vec<u8> = sqlx::query_scalar("SELECT words FROM transcript_lines WHERE call_id = 'call-a' AND seq = 2").fetch_one(pool).await.unwrap();
    let at = Place { table: "transcript_lines", row: "call-a/2", column: "words" };
    assert_eq!(&*open_text(&AwsLc, &key(), &at, &words).unwrap(), "Jordan Ellis, March 12, 1984");
    let mary = users::list(&store).await.unwrap().into_iter().find(|u| u.id == id).unwrap();
    assert_eq!((mary.email.as_str(), mary.name.as_str(), mary.phone.as_str()), ("mary@example.com", "Mary Lee", "+15552345678"));
    assert_eq!(users::photo(&store, &id).await.unwrap().unwrap().as_slice(), webp.as_slice());

    store.pool().close().await;
    sqlx::query(&format!("DROP DATABASE {name}")).execute(&mut admin).await.unwrap();
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
    sqlx::query(&format!("DROP DATABASE IF EXISTS {name}")).execute(&mut admin).await.unwrap();
    sqlx::query(&format!("CREATE DATABASE {name}")).execute(&mut admin).await.unwrap();
    let base = url.rsplit_once('/').unwrap().0;
    let store = Store::connect(&format!("{base}/{name}"), key()).await.unwrap();
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

    store.pool().close().await;
    sqlx::query(&format!("DROP DATABASE {name}")).execute(&mut admin).await.unwrap();
}
