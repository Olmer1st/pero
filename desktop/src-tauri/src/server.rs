//! Talks to the Pero server for the page. Requests go out from here, not from the web view: they use a TLS client
//! that trusts exactly one certificate authority (the owner's home CA, built into the app), and the device token is
//! added here from memory or the keychain, so the page never holds it.

use std::{sync::Mutex, time::Duration};

use reqwest::Client;
use serde::Serialize;
use serde_json::Value;

use crate::{rules, secrets, settings};

/// The home CA's public certificate, copied in at build time (see build.rs).
const CA_PEM: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/ca.crt"));

/// A response must be small enough to hold (an imported book can be a few megabytes).
const MAX_RESPONSE: usize = 64 * 1024 * 1024;

pub struct State {
    client: Client,
    settings: settings::Store,
    address: Mutex<Option<String>>,
    token: Mutex<Option<String>>,
}

#[derive(Serialize)]
pub struct Response {
    pub status: u16,
    pub body: String,
}

#[derive(Serialize)]
pub struct Found {
    pub address: String,
    pub version: String,
}

#[derive(Serialize)]
pub struct Session {
    /// The device as the server describes it (`{id, name}`).
    pub device: Value,
    /// False when the token could not be put in the keychain: it then lasts until the app closes.
    pub remembered: bool,
}

impl State {
    pub fn new(config_dir: std::path::PathBuf) -> Result<State, String> {
        let ca = reqwest::Certificate::from_pem(CA_PEM)
            .map_err(|e| format!("the built-in CA certificate is unreadable: {e}"))?;
        let client = Client::builder()
            .tls_certs_only([ca])
            .https_only(true)
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(4))
            .timeout(Duration::from_secs(60))
            .user_agent(concat!("pero-desktop/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| format!("cannot start the network client: {e}"))?;
        Ok(State {
            client,
            settings: settings::Store::new(config_dir),
            address: Mutex::new(None),
            token: Mutex::new(None),
        })
    }

    fn base(&self) -> Result<String, String> {
        self.address
            .lock()
            .unwrap()
            .clone()
            .map(|a| format!("https://{a}"))
            .ok_or_else(|| "The server has not been found yet.".to_string())
    }

    async fn send(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<String>,
        token: Option<&str>,
    ) -> Result<Response, String> {
        rules::api_path(path)?;
        let mut request = self
            .client
            .request(method, format!("{}{path}", self.base()?))
            .header("Accept", "application/json");
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        if let Some(body) = body {
            request = request
                .header("Content-Type", "application/json")
                .body(body);
        }
        let response = request.send().await.map_err(|e| describe(&e))?;
        let status = response.status().as_u16();
        let bytes = response.bytes().await.map_err(|e| describe(&e))?;
        if bytes.len() > MAX_RESPONSE {
            return Err("The server's answer is too large.".into());
        }
        Ok(Response {
            status,
            body: String::from_utf8_lossy(&bytes).into_owned(),
        })
    }

    /// Tries the remembered and built-in addresses in turn; the first that answers `/health` over the pinned CA wins.
    pub async fn discover(&self) -> Result<Found, String> {
        let mut last = String::from("No address to try.");
        for candidate in self.settings.candidates() {
            *self.address.lock().unwrap() = Some(candidate.clone());
            match self.send(reqwest::Method::GET, "/health", None, None).await {
                Ok(r) if r.status == 200 => {
                    if let Some(version) = serde_json::from_str::<Value>(&r.body)
                        .ok()
                        .and_then(|v| v.get("version").and_then(Value::as_str).map(str::to_string))
                    {
                        return Ok(Found {
                            address: candidate,
                            version,
                        });
                    }
                    last = format!("{candidate} answered, but it is not a Pero server.");
                }
                Ok(r) => last = format!("{candidate} answered with status {}.", r.status),
                Err(e) => last = format!("{candidate}: {e}"),
            }
        }
        *self.address.lock().unwrap() = None;
        Err(format!("The Pero server was not found. {last}"))
    }

    /// An address typed by hand: checked, tried, and remembered only when it really is a Pero server.
    pub async fn use_address(&self, input: &str) -> Result<Found, String> {
        let address = rules::server_address(input)?;
        *self.address.lock().unwrap() = Some(address.clone());
        let r = self
            .send(reqwest::Method::GET, "/health", None, None)
            .await
            .inspect_err(|_| {
                *self.address.lock().unwrap() = None;
            })?;
        let version = (r.status == 200)
            .then(|| serde_json::from_str::<Value>(&r.body).ok())
            .flatten()
            .and_then(|v| v.get("version").and_then(Value::as_str).map(str::to_string));
        match version {
            Some(version) => {
                self.settings.remember(&address)?;
                Ok(Found { address, version })
            }
            None => {
                *self.address.lock().unwrap() = None;
                Err(format!("{address} answered, but it is not a Pero server."))
            }
        }
    }

    pub async fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<String>,
    ) -> Result<Response, String> {
        let method = rules::method(method)?;
        let token = self.token.lock().unwrap().clone();
        // The health check is open; everything else carries the device token if there is one.
        let token = if path == "/health" { None } else { token };
        self.send(method, path, body, token.as_deref()).await
    }

    /// Checks the token with the server; only a token the server accepts is kept (and put in the keychain).
    pub async fn sign_in(&self, token: &str) -> Result<Session, String> {
        let token = token.trim();
        if !rules::token_shape(token) {
            return Err("That does not look like a device token.".into());
        }
        let r = self
            .send(reqwest::Method::GET, "/api/v1/me", None, Some(token))
            .await?;
        if r.status == 401 {
            return Err("That token was not accepted.".into());
        }
        if r.status != 200 {
            return Err(format!("The server answered with status {}.", r.status));
        }
        let device = serde_json::from_str(&r.body)
            .map_err(|_| "The server's answer was not understood.".to_string())?;
        *self.token.lock().unwrap() = Some(token.to_string());
        let remembered = secrets::save(token).is_ok();
        Ok(Session { device, remembered })
    }

    /// At start: signs in again with the token from the keychain, if there is one and the server still accepts it.
    pub async fn resume(&self) -> Result<Option<Session>, String> {
        let Some(token) = secrets::load().ok().flatten() else {
            return Ok(None);
        };
        let r = self
            .send(reqwest::Method::GET, "/api/v1/me", None, Some(&token))
            .await?;
        match r.status {
            200 => {
                let device = serde_json::from_str(&r.body)
                    .map_err(|_| "The server's answer was not understood.".to_string())?;
                *self.token.lock().unwrap() = Some(token);
                Ok(Some(Session {
                    device,
                    remembered: true,
                }))
            }
            401 => {
                let _ = secrets::clear(); // revoked or wrong: forget it
                Ok(None)
            }
            other => Err(format!("The server answered with status {other}.")),
        }
    }

    pub fn sign_out(&self) {
        *self.token.lock().unwrap() = None;
        let _ = secrets::clear();
    }
}

/// A short, human reason (never the URL or headers).
fn describe(e: &reqwest::Error) -> String {
    if e.is_timeout() {
        "The server did not answer in time.".into()
    } else if e.is_connect() {
        "Cannot connect to the server (is it on, and are you at home?).".into()
    } else {
        "The request failed.".into()
    }
}
