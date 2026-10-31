//! The only setting the app keeps: server addresses the owner typed in by hand. Not secret; stored as JSON in the
//! app's config folder. The device token is never written here (it goes to the system keychain).

use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};

/// Tried after the addresses typed by hand. The Tailscale name will be one more entry when Tailscale is used.
pub const DEFAULT_SERVERS: &[&str] = &["writer.local:8443"];

#[derive(Default, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub servers: Vec<String>,
}

pub struct Store {
    path: PathBuf,
}

impl Store {
    pub fn new(dir: PathBuf) -> Store {
        Store {
            path: dir.join("settings.json"),
        }
    }

    pub fn load(&self) -> Settings {
        fs::read_to_string(&self.path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    /// Puts `address` first (most recently used), without duplicates.
    pub fn remember(&self, address: &str) -> Result<(), String> {
        let mut s = self.load();
        s.servers.retain(|a| a != address);
        s.servers.insert(0, address.to_string());
        s.servers.truncate(5);
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir).map_err(|e| format!("cannot save the setting: {e}"))?;
        }
        let text = serde_json::to_string_pretty(&s).map_err(|e| e.to_string())?;
        fs::write(&self.path, text).map_err(|e| format!("cannot save the setting: {e}"))
    }

    /// Hand-typed addresses first, then the built-in ones.
    pub fn candidates(&self) -> Vec<String> {
        let mut all = self.load().servers;
        for d in DEFAULT_SERVERS {
            if !all.iter().any(|a| a == d) {
                all.push((*d).to_string());
            }
        }
        all
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remembered_addresses_come_first_without_duplicates() {
        let dir =
            std::env::temp_dir().join(format!("pero-desktop-settings-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = Store::new(dir.clone());
        assert_eq!(store.candidates(), ["writer.local:8443"]);
        store.remember("10.0.0.106:8443").unwrap();
        store.remember("other.example:8443").unwrap();
        store.remember("10.0.0.106:8443").unwrap();
        assert_eq!(
            store.candidates(),
            ["10.0.0.106:8443", "other.example:8443", "writer.local:8443"]
        );
        let _ = fs::remove_dir_all(dir);
    }
}
