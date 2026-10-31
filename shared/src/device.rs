use serde::Serialize;
use ts_rs::TS;

/// The device a request was made by.
#[derive(TS, Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Device {
    pub id: String,
    pub name: String,
}

#[derive(TS, Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub last_seen_at: Option<String>,
    pub revoked_at: Option<String>,
}
