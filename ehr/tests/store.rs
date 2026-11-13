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
