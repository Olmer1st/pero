//! SQLite access: connection settings and versioned migrations (sqlx).
//! The database holds structure and identifiers only; the prose lives in the book repositories.

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use std::{path::Path, time::Duration};

/// Migrations from `core/migrations`, embedded at build time. Forward-only, applied in version order.
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

fn base_options() -> SqliteConnectOptions {
    SqliteConnectOptions::new()
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(5))
}

/// Opens (and creates, if missing) the database file, then applies pending migrations.
pub async fn open(path: &Path) -> Result<SqlitePool, String> {
    let options = base_options()
        .filename(path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal);
    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await
        .map_err(|e| format!("cannot open database {}: {e}", path.display()))?;
    migrate(&pool).await?;
    Ok(pool)
}

/// Applies all pending migrations. Running it again changes nothing.
pub async fn migrate(pool: &SqlitePool) -> Result<(), String> {
    MIGRATOR
        .run(pool)
        .await
        .map_err(|e| format!("database migration failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::Row;

    /// A private in-memory database (one connection, kept for the whole test).
    async fn empty_db() -> SqlitePool {
        SqlitePoolOptions::new()
            .max_connections(1)
            .idle_timeout(None)
            .max_lifetime(None)
            .connect_with(base_options().in_memory(true))
            .await
            .unwrap()
    }

    async fn table_names(pool: &SqlitePool) -> Vec<String> {
        sqlx::query(
            "SELECT name FROM sqlite_master WHERE type = 'table' \
             AND name NOT LIKE 'sqlite_%' AND name <> '_sqlx_migrations' ORDER BY name",
        )
        .fetch_all(pool)
        .await
        .unwrap()
        .iter()
        .map(|r| r.get::<String, _>("name"))
        .collect()
    }

    async fn applied_versions(pool: &SqlitePool) -> Vec<i64> {
        sqlx::query("SELECT version FROM _sqlx_migrations WHERE success = 1 ORDER BY version")
            .fetch_all(pool)
            .await
            .unwrap()
            .iter()
            .map(|r| r.get::<i64, _>("version"))
            .collect()
    }

    /// Runs one statement built by a test from its own fixtures (never from request data).
    async fn exec(pool: &SqlitePool, sql: &str) -> Result<(), sqlx::Error> {
        sqlx::query(sqlx::AssertSqlSafe(sql.to_string()))
            .execute(pool)
            .await
            .map(|_| ())
    }

    /// Fixture rows used by several tests: one book, chapter, scene, two passages, one entity.
    async fn seed(pool: &SqlitePool) {
        let h = "a".repeat(64);
        for sql in [
            "INSERT INTO books (id, title, language) VALUES ('bk_1', 'Тихий Дон', 'ru')".to_string(),
            "INSERT INTO chapters (id, book_id, position, path) VALUES ('ch_1', 'bk_1', 0, 'chapters/01/01.md')".to_string(),
            "INSERT INTO scenes (id, book_id, chapter_id, position, path, language) VALUES ('sc_1', 'bk_1', 'ch_1', 0, 'chapters/01/scenes/01.md', 'ru')".to_string(),
            format!("INSERT INTO passages (id, book_id, scene_id, position, text_hash) VALUES ('k7m2x9qa', 'bk_1', 'sc_1', 0, '{h}')"),
            format!("INSERT INTO passages (id, book_id, scene_id, position, text_hash) VALUES ('p3n8d5wz', 'bk_1', 'sc_1', 1, '{h}')"),
        ] {
            exec(pool, &sql).await.unwrap();
        }
    }

    const ALL_TABLES: [&str; 12] = [
        "books",
        "chapters",
        "entities",
        "entity_aliases",
        "fact_evidence",
        "fact_status_history",
        "facts",
        "passages",
        "relationships",
        "scenes",
        "timeline_event_participants",
        "timeline_events",
    ];

    #[tokio::test]
    async fn migrations_apply_to_an_empty_database() {
        let pool = empty_db().await;
        assert!(table_names(&pool).await.is_empty());
        migrate(&pool).await.unwrap();
        assert_eq!(table_names(&pool).await, ALL_TABLES);
        let expected: Vec<i64> = MIGRATOR.iter().map(|m| m.version).collect();
        assert_eq!(applied_versions(&pool).await, expected);
        let fk_problems = sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&pool)
            .await
            .unwrap();
        assert!(fk_problems.is_empty());
        let integrity: String = sqlx::query("PRAGMA integrity_check")
            .fetch_one(&pool)
            .await
            .unwrap()
            .get(0);
        assert_eq!(integrity, "ok");
    }

    #[tokio::test]
    async fn migrating_again_changes_nothing() {
        let pool = empty_db().await;
        migrate(&pool).await.unwrap();
        let before = applied_versions(&pool).await;
        migrate(&pool).await.unwrap();
        assert_eq!(applied_versions(&pool).await, before);
    }

    #[tokio::test]
    async fn an_edited_applied_migration_is_refused() {
        let pool = empty_db().await;
        migrate(&pool).await.unwrap();
        exec(&pool, "UPDATE _sqlx_migrations SET checksum = x'00' WHERE version = (SELECT min(version) FROM _sqlx_migrations)")
            .await
            .unwrap();
        let err = migrate(&pool).await.unwrap_err();
        assert!(err.contains("migration"), "{err}");
    }

    #[tokio::test]
    async fn migrations_roll_forward_over_existing_data() {
        let pool = empty_db().await;
        let first = MIGRATOR.iter().next().unwrap().version;
        MIGRATOR.run_to(first, &pool).await.unwrap();
        assert_eq!(
            table_names(&pool).await,
            ["books", "chapters", "passages", "scenes"]
        );
        seed(&pool).await;

        migrate(&pool).await.unwrap();

        assert_eq!(table_names(&pool).await, ALL_TABLES);
        let n: i64 = sqlx::query("SELECT count(*) FROM passages")
            .fetch_one(&pool)
            .await
            .unwrap()
            .get(0);
        assert_eq!(n, 2);
        let title: String = sqlx::query("SELECT title FROM books WHERE id = 'bk_1'")
            .fetch_one(&pool)
            .await
            .unwrap()
            .get(0);
        assert_eq!(title, "Тихий Дон");
    }

    #[tokio::test]
    async fn opening_a_file_creates_it_once_and_keeps_the_data() {
        let dir = std::env::temp_dir().join(format!("pero-db-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("pero.db");
        {
            let pool = open(&path).await.unwrap();
            seed(&pool).await;
            pool.close().await;
        }
        let pool = open(&path).await.unwrap();
        let n: i64 = sqlx::query("SELECT count(*) FROM passages")
            .fetch_one(&pool)
            .await
            .unwrap()
            .get(0);
        assert_eq!(n, 2);
        let mode: String = sqlx::query("PRAGMA journal_mode")
            .fetch_one(&pool)
            .await
            .unwrap()
            .get(0);
        assert_eq!(mode, "wal");
        let fk: i64 = sqlx::query("PRAGMA foreign_keys")
            .fetch_one(&pool)
            .await
            .unwrap()
            .get(0);
        assert_eq!(fk, 1);
        pool.close().await;
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn the_schema_rejects_invalid_rows() {
        let pool = empty_db().await;
        migrate(&pool).await.unwrap();
        seed(&pool).await;
        let h = "b".repeat(64);
        let bad = [
            // unknown language, short hash, orphan reference
            "INSERT INTO books (id, title, language) VALUES ('bk_2', 'X', 'de')".to_string(),
            "INSERT INTO passages (id, book_id, scene_id, position, text_hash) VALUES ('zz', 'bk_1', 'sc_1', 2, 'short')".to_string(),
            "INSERT INTO chapters (id, book_id, position, path) VALUES ('ch_9', 'bk_missing', 0, 'x.md')".to_string(),
            // a removed passage needs a removal time; only a removed passage can be merged
            format!("INSERT INTO passages (id, book_id, scene_id, position, text_hash, status) VALUES ('rm1', 'bk_1', 'sc_1', 3, '{h}', 'removed')"),
            format!("INSERT INTO passages (id, book_id, scene_id, position, text_hash, merged_into) VALUES ('mg1', 'bk_1', 'sc_1', 3, '{h}', 'k7m2x9qa')"),
            // entity kind, relationship to itself
            "INSERT INTO entities (id, book_id, kind, name) VALUES ('en_x', 'bk_1', 'dragon', 'Z')".to_string(),
        ];
        for sql in &bad {
            assert!(exec(&pool, sql).await.is_err(), "accepted: {sql}");
        }

        // the valid counterparts of the rejected rows are accepted
        exec(&pool, &format!("INSERT INTO passages (id, book_id, scene_id, position, text_hash, status, removed_at, merged_into) VALUES ('rm1', 'bk_1', 'sc_1', 3, '{h}', 'removed', '2026-10-28T00:00:00Z', 'k7m2x9qa')")).await.unwrap();
        exec(
            &pool,
            "INSERT INTO books (id, title, language) VALUES ('bk_2', 'Book', 'en')",
        )
        .await
        .unwrap();
        exec(&pool, "INSERT INTO entities (id, book_id, kind, name) VALUES ('en_1', 'bk_1', 'character', 'Григорий')").await.unwrap();
        exec(&pool, "INSERT INTO entities (id, book_id, kind, name) VALUES ('en_2', 'bk_1', 'place', 'Москва')").await.unwrap();
        exec(&pool, "INSERT INTO relationships (id, book_id, from_entity_id, to_entity_id, kind) VALUES ('rl_0', 'bk_1', 'en_1', 'en_2', 'lives in')").await.unwrap();
        assert!(exec(&pool, "INSERT INTO relationships (id, book_id, from_entity_id, to_entity_id, kind) VALUES ('rl_1', 'bk_1', 'en_1', 'en_1', 'self')").await.is_err());

        // canon rules in the schema: no author_approved without an approval record; unknown status; stale reason only when stale
        assert!(exec(&pool, "INSERT INTO facts (id, book_id, text, status, origin) VALUES ('ft_1', 'bk_1', 'Ф', 'author_approved', 'ai')").await.is_err());
        assert!(exec(&pool, "INSERT INTO facts (id, book_id, text, status, origin) VALUES ('ft_2', 'bk_1', 'Ф', 'canon', 'ai')").await.is_err());
        assert!(exec(&pool, "INSERT INTO facts (id, book_id, text, status, origin, stale_reason) VALUES ('ft_3', 'bk_1', 'Ф', 'ai_hypothesis', 'ai', 'x')").await.is_err());
        exec(&pool, "INSERT INTO facts (id, book_id, text, status, origin) VALUES ('ft_4', 'bk_1', 'Григорий жил в Москве', 'ai_hypothesis', 'ai')").await.unwrap();
        exec(&pool, "INSERT INTO facts (id, book_id, text, status, origin, approved_by, approved_at) VALUES ('ft_5', 'bk_1', 'Ф', 'author_approved', 'author', 'laptop', '2026-10-28T00:00:00Z')").await.unwrap();

        // evidence and history must point at real rows; a cited passage cannot be deleted
        assert!(exec(&pool, "INSERT INTO fact_evidence (fact_id, passage_id, text_hash) VALUES ('ft_4', 'nope', 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb')").await.is_err());
        exec(&pool, &format!("INSERT INTO fact_evidence (fact_id, passage_id, text_hash) VALUES ('ft_4', 'k7m2x9qa', '{h}')")).await.unwrap();
        assert!(
            exec(&pool, "DELETE FROM passages WHERE id = 'k7m2x9qa'")
                .await
                .is_err()
        );
        assert!(exec(&pool, "INSERT INTO fact_status_history (fact_id, from_status, to_status, changed_by) VALUES ('ft_4', NULL, 'ai_hypothesis', 'ai:test')").await.is_ok());
        assert!(exec(&pool, "INSERT INTO fact_status_history (fact_id, to_status, changed_by) VALUES ('ft_4', 'maybe', 'x')").await.is_err());
    }

    #[tokio::test]
    async fn russian_text_is_stored_and_returned_byte_for_byte() {
        let pool = empty_db().await;
        migrate(&pool).await.unwrap();
        seed(&pool).await;
        let name = "Ёжик «Тихий» — друг";
        sqlx::query("INSERT INTO entities (id, book_id, kind, name) VALUES ('en_1', 'bk_1', 'character', ?)")
            .bind(name)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO entity_aliases (entity_id, alias) VALUES ('en_1', ?)")
            .bind("Ёж")
            .execute(&pool)
            .await
            .unwrap();
        let back: String = sqlx::query("SELECT name FROM entities WHERE id = 'en_1'")
            .fetch_one(&pool)
            .await
            .unwrap()
            .get(0);
        assert_eq!(back.as_bytes(), name.as_bytes());
        let alias: String =
            sqlx::query("SELECT alias FROM entity_aliases WHERE entity_id = 'en_1'")
                .fetch_one(&pool)
                .await
                .unwrap()
                .get(0);
        assert_eq!(alias, "Ёж");
    }
}
