//! Test helper: a real router over a throw-away data folder, driven with in-memory requests.

use std::sync::atomic::{AtomicUsize, Ordering};

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::Value;
use tower::ServiceExt;

use crate::{api, db, device, store::Store};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Name of the device every test request is made as, unless it says otherwise.
pub const DEVICE_NAME: &str = "laptop";

pub struct Server {
    pub store: std::sync::Arc<Store>,
    /// Token of the default device `DEVICE_NAME`.
    pub token: String,
    app: Router,
    dir: std::path::PathBuf,
}

impl Server {
    pub async fn new(tag: &str) -> Server {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir =
            std::env::temp_dir().join(format!("pero-api-test-{}-{tag}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let pool = db::open(&dir.join("pero.db")).await.unwrap();
        let store = Store::new(pool, dir.join("books")).unwrap();
        let (_, token) = device::add(store.pool(), DEVICE_NAME).await.unwrap();
        Server {
            app: api::router(store.clone()),
            store,
            token,
            dir,
        }
    }

    /// Sends one request as the default device; the answer body is parsed as JSON (`Null` when
    /// empty or not JSON).
    pub async fn call(&self, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
        let token = self.token.clone();
        self.call_with(Some(&token), method, uri, body).await
    }

    /// Sends one request with the given bearer token, or none.
    pub async fn call_with(
        &self,
        token: Option<&str>,
        method: &str,
        uri: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let (status, _, json) = self
            .raw(token.map(|t| format!("Bearer {t}")), method, uri, body)
            .await;
        (status, json)
    }

    /// Sends one request with an arbitrary `Authorization` header value; also returns the headers.
    pub async fn raw(
        &self,
        authorization: Option<String>,
        method: &str,
        uri: &str,
        body: Option<Value>,
    ) -> (StatusCode, axum::http::HeaderMap, Value) {
        let mut req = Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json");
        if let Some(a) = authorization {
            req = req.header("authorization", a);
        }
        let req = req
            .body(body.map_or_else(Body::empty, |b| Body::from(b.to_string())))
            .unwrap();
        let res = self.app.clone().oneshot(req).await.unwrap();
        let (status, headers) = (res.status(), res.headers().clone());
        let bytes = axum::body::to_bytes(res.into_body(), 64 << 20)
            .await
            .unwrap();
        (
            status,
            headers,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    /// Folder that holds the book repositories.
    pub fn dir_of_books(&self) -> std::path::PathBuf {
        self.dir.join("books")
    }

    /// Registers another device and returns its token.
    pub async fn add_device(&self, name: &str) -> String {
        device::add(self.store.pool(), name).await.unwrap().1
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
