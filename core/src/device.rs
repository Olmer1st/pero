//! Devices and their API tokens. A token is 256 random bits shown once, as `pero_` and 64 hex
//! characters; only its SHA-256 hash is stored. Tokens are looked up by hash on every request, so a
//! revocation takes effect immediately. Devices are revoked, never deleted.

use serde::Serialize;
use sqlx::{Row, SqlitePool};

use crate::{ids, manuscript, store::StoreError};

const TOKEN_PREFIX: &str = "pero_";
const TOKEN_HEX_CHARS: usize = 64;
const MAX_NAME_CHARS: usize = 60;
fn invalid(m: impl Into<String>) -> StoreError {
    StoreError::Invalid(m.into())
}

/// The device a request was made by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Device {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub last_seen_at: Option<String>,
    pub revoked_at: Option<String>,
}

macro_rules! info_columns {
    () => {
        "id, name, created_at, last_seen_at, revoked_at"
    };
}

/// A fresh token: 32 random bytes as `pero_` plus 64 lowercase hex characters.
pub fn new_token() -> String {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).expect("operating system random source");
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("{TOKEN_PREFIX}{hex}")
}

/// The stored form of a token.
pub fn token_hash(token: &str) -> String {
    manuscript::sha256_hex(token.as_bytes())
}

/// True for the shape of a token; anything else is rejected without touching the database.
pub fn looks_like_token(token: &str) -> bool {
    token.strip_prefix(TOKEN_PREFIX).is_some_and(|hex| {
        hex.len() == TOKEN_HEX_CHARS && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    })
}

fn clean_name(name: &str) -> Result<String, StoreError> {
    let n = manuscript::clean_field("name", name).map_err(invalid)?;
    if n.is_empty() {
        return Err(invalid("name cannot be empty"));
    }
    if n.chars().count() > MAX_NAME_CHARS {
        return Err(invalid(format!(
            "name is longer than {MAX_NAME_CHARS} characters"
        )));
    }
    if n.contains(['<', '>']) {
        return Err(invalid("name cannot contain < or >"));
    }
    Ok(n)
}

/// Registers a device and returns it with its token. The token is not stored and cannot be shown again.
pub async fn add(pool: &SqlitePool, name: &str) -> Result<(DeviceInfo, String), StoreError> {
    let name = clean_name(name)?;
    let taken: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM devices WHERE name = ?")
        .bind(&name)
        .fetch_optional(pool)
        .await?;
    if taken.is_some() {
        return Err(StoreError::Exists(
            "a device with this name exists (revoked names are not reused)".into(),
        ));
    }
    let id = ids::new_id(ids::DEVICE);
    let token = new_token();
    sqlx::query("INSERT INTO devices (id, name, token_hash) VALUES (?, ?, ?)")
        .bind(&id)
        .bind(&name)
        .bind(token_hash(&token))
        .execute(pool)
        .await?;
    Ok((get(pool, &id).await?, token))
}

pub async fn get(pool: &SqlitePool, id: &str) -> Result<DeviceInfo, StoreError> {
    sqlx::query_as(concat!(
        "SELECT ",
        info_columns!(),
        " FROM devices WHERE id = ?"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| StoreError::NotFound("device not found".into()))
}

/// All devices, oldest first, revoked ones included.
pub async fn list(pool: &SqlitePool) -> Result<Vec<DeviceInfo>, StoreError> {
    Ok(sqlx::query_as(concat!(
        "SELECT ",
        info_columns!(),
        " FROM devices ORDER BY created_at, id"
    ))
    .fetch_all(pool)
    .await?)
}

pub async fn count_active(pool: &SqlitePool) -> Result<i64, StoreError> {
    let (n,): (i64,) = sqlx::query_as("SELECT count(*) FROM devices WHERE revoked_at IS NULL")
        .fetch_one(pool)
        .await?;
    Ok(n)
}

/// Revokes a device by ID or by name. Other devices are not affected; the token stops working at once.
pub async fn revoke(pool: &SqlitePool, id_or_name: &str) -> Result<DeviceInfo, StoreError> {
    let row: Option<(String, Option<String>)> =
        sqlx::query_as("SELECT id, revoked_at FROM devices WHERE id = ?1 OR name = ?1")
            .bind(id_or_name)
            .fetch_optional(pool)
            .await?;
    let (id, revoked_at) = row.ok_or_else(|| StoreError::NotFound("device not found".into()))?;
    if revoked_at.is_some() {
        return Err(StoreError::Transition(
            "the device is already revoked".into(),
        ));
    }
    sqlx::query(
        "UPDATE devices SET revoked_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
    )
    .bind(&id)
    .execute(pool)
    .await?;
    get(pool, &id).await
}

/// The device a token belongs to, if the token is valid and not revoked.
pub async fn authenticate(pool: &SqlitePool, token: &str) -> Result<Option<Device>, StoreError> {
    if !looks_like_token(token) {
        return Ok(None);
    }
    let row =
        sqlx::query("SELECT id, name FROM devices WHERE token_hash = ? AND revoked_at IS NULL")
            .bind(token_hash(token))
            .fetch_optional(pool)
            .await?;
    let Some(row) = row else { return Ok(None) };
    let device = Device {
        id: row.get("id"),
        name: row.get("name"),
    };
    // Refreshed at most once a minute per device, so reading does not mean a database write per request.
    sqlx::query(
        "UPDATE devices SET last_seen_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ? \
         AND (last_seen_at IS NULL OR last_seen_at < strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-60 seconds'))",
    )
    .bind(&device.id)
    .execute(pool)
    .await?;
    Ok(Some(device))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    async fn pool() -> SqlitePool {
        db::open_memory().await
    }

    #[test]
    fn tokens_have_a_fixed_shape_and_are_random() {
        let (a, b) = (new_token(), new_token());
        assert!(looks_like_token(&a) && looks_like_token(&b));
        assert_eq!(a.len(), "pero_".len() + 64);
        assert_ne!(a, b);
        let hash = token_hash(&a);
        assert_eq!(hash.len(), 64);
        assert!(!hash.contains(&a) && hash != token_hash(&b));
        for bad in [
            "",
            "pero_",
            "pero_abc",
            &a.to_uppercase(),
            &a.replace("pero_", "xero_"),
            &format!("{a}0"),
            &a[..a.len() - 1],
            &format!("pero_{}g", &a[5..a.len() - 1]),
        ] {
            assert!(!looks_like_token(bad), "{bad}");
        }
    }

    #[tokio::test]
    async fn only_the_hash_of_a_token_is_stored() {
        let pool = pool().await;
        let (info, token) = add(&pool, "laptop").await.unwrap();
        assert!(
            info.id.starts_with("dv_") && info.revoked_at.is_none() && info.last_seen_at.is_none()
        );
        let row = sqlx::query("SELECT id || '|' || name || '|' || token_hash || '|' || created_at AS everything, token_hash FROM devices")
            .fetch_one(&pool)
            .await
            .unwrap();
        let everything: String = row.get("everything");
        assert!(
            !everything.contains(&token),
            "the token itself is not stored anywhere"
        );
        assert_eq!(row.get::<String, _>("token_hash"), token_hash(&token));
        // the JSON shown to clients carries no hash either
        let json = serde_json::to_string(&info).unwrap();
        assert!(!json.contains("token") && !json.contains(&token_hash(&token)));
    }

    #[tokio::test]
    async fn a_token_works_until_its_device_is_revoked_and_other_devices_are_unaffected() {
        let pool = pool().await;
        let (laptop, t1) = add(&pool, "laptop").await.unwrap();
        let (_, t2) = add(&pool, "phone").await.unwrap();
        assert_eq!(
            authenticate(&pool, &t1).await.unwrap().unwrap().name,
            "laptop"
        );
        assert_eq!(
            authenticate(&pool, &t2).await.unwrap().unwrap().name,
            "phone"
        );
        assert_eq!(count_active(&pool).await.unwrap(), 2);

        let revoked = revoke(&pool, "phone").await.unwrap();
        assert!(revoked.revoked_at.is_some());
        assert_eq!(
            authenticate(&pool, &t2).await.unwrap(),
            None,
            "a revoked token stops working at once"
        );
        assert_eq!(
            authenticate(&pool, &t1).await.unwrap().unwrap().id,
            laptop.id
        );
        assert_eq!(count_active(&pool).await.unwrap(), 1);
        assert!(matches!(
            revoke(&pool, "phone").await,
            Err(StoreError::Transition(_))
        ));
        assert!(matches!(
            revoke(&pool, "nobody").await,
            Err(StoreError::NotFound(_))
        ));

        // revoking by ID works as well; the list keeps revoked devices
        revoke(&pool, &laptop.id).await.unwrap();
        assert_eq!(authenticate(&pool, &t1).await.unwrap(), None);
        let all = list(&pool).await.unwrap();
        assert_eq!(
            all.iter().map(|d| d.name.as_str()).collect::<Vec<_>>(),
            ["laptop", "phone"]
        );
        assert!(all.iter().all(|d| d.revoked_at.is_some()));
    }

    #[tokio::test]
    async fn wrong_tokens_never_authenticate() {
        let pool = pool().await;
        let (_, token) = add(&pool, "laptop").await.unwrap();
        let mut flipped = token.clone().into_bytes();
        let last = flipped.len() - 1;
        flipped[last] = if flipped[last] == b'0' { b'1' } else { b'0' };
        for bad in [
            String::from_utf8(flipped).unwrap(),
            new_token(),
            "pero_".into(),
            String::new(),
            token.to_uppercase(),
        ] {
            assert_eq!(authenticate(&pool, &bad).await.unwrap(), None, "{bad}");
        }
    }

    #[tokio::test]
    async fn names_are_checked_and_never_reused() {
        let pool = pool().await;
        for bad in ["", "   ", "a\nb", "<script>", "я".repeat(61).as_str()] {
            assert!(
                matches!(add(&pool, bad).await, Err(StoreError::Invalid(_))),
                "{bad:?}"
            );
        }
        add(&pool, "Ноутбук Вадима").await.unwrap();
        assert!(matches!(
            add(&pool, "Ноутбук Вадима").await,
            Err(StoreError::Exists(_))
        ));
        revoke(&pool, "Ноутбук Вадима").await.unwrap();
        assert!(
            matches!(
                add(&pool, "Ноутбук Вадима").await,
                Err(StoreError::Exists(_))
            ),
            "a revoked name stays reserved so history is never ambiguous"
        );
    }

    #[tokio::test]
    async fn last_seen_is_recorded_but_not_on_every_request() {
        let pool = pool().await;
        let (info, token) = add(&pool, "laptop").await.unwrap();
        assert_eq!(info.last_seen_at, None);
        authenticate(&pool, &token).await.unwrap();
        let first = get(&pool, &info.id)
            .await
            .unwrap()
            .last_seen_at
            .expect("set on first use");
        authenticate(&pool, &token).await.unwrap();
        assert_eq!(
            get(&pool, &info.id).await.unwrap().last_seen_at.as_deref(),
            Some(first.as_str()),
            "refreshed at most once a minute"
        );
        sqlx::query("UPDATE devices SET last_seen_at = '2000-01-01T00:00:00.000Z'")
            .execute(&pool)
            .await
            .unwrap();
        authenticate(&pool, &token).await.unwrap();
        assert_ne!(
            get(&pool, &info.id).await.unwrap().last_seen_at.as_deref(),
            Some("2000-01-01T00:00:00.000Z")
        );
    }
}
