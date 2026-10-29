//! The device token lives in the operating system's keychain (the Secret Service on Linux: GNOME Keyring or
//! KWallet), never in a file, in the page's storage or in the settings.

use keyring::{Entry, Error};

const SERVICE: &str = "pero-desktop";
const ACCOUNT: &str = "device-token";

fn entry() -> Result<Entry, String> {
    Entry::new(SERVICE, ACCOUNT).map_err(|e| format!("the system keychain is not available: {e}"))
}

/// Writes, reads and deletes a throw-away entry, to show that the keychain works without touching the real token.
pub fn probe() -> Result<(), String> {
    let probe = Entry::new(SERVICE, "self-test")
        .map_err(|e| format!("the system keychain is not available: {e}"))?;
    probe
        .set_password("probe")
        .map_err(|e| format!("cannot write the keychain: {e}"))?;
    let back = probe
        .get_password()
        .map_err(|e| format!("cannot read the keychain: {e}"))?;
    probe
        .delete_credential()
        .map_err(|e| format!("cannot clear the keychain: {e}"))?;
    if back == "probe" {
        Ok(())
    } else {
        Err("the keychain returned something else".into())
    }
}

pub fn load() -> Result<Option<String>, String> {
    match entry()?.get_password() {
        Ok(token) => Ok(Some(token)),
        Err(Error::NoEntry) => Ok(None),
        Err(e) => Err(format!("cannot read the keychain: {e}")),
    }
}

pub fn save(token: &str) -> Result<(), String> {
    entry()?
        .set_password(token)
        .map_err(|e| format!("cannot write the keychain: {e}"))
}

pub fn clear() -> Result<(), String> {
    match entry()?.delete_credential() {
        Ok(()) | Err(Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("cannot clear the keychain: {e}")),
    }
}
