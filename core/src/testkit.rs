//! Test helper: a real router over a throw-away data folder, driven with in-memory requests.

use std::sync::atomic::{AtomicUsize, Ordering};

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::Value;
use tower::ServiceExt;

use crate::{api, db, store::Store};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

pub struct Server {
    pub store: std::sync::Arc<Store>,
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
        Server {
            app: api::router(store.clone()),
            store,
            dir,
        }
    }

    /// Sends one request; the answer body is parsed as JSON (`Null` when empty or not JSON).
    pub async fn call(&self, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
        let req = Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .body(body.map_or_else(Body::empty, |b| Body::from(b.to_string())))
            .unwrap();
        let res = self.app.clone().oneshot(req).await.unwrap();
        let status = res.status();
        let bytes = axum::body::to_bytes(res.into_body(), 64 << 20)
            .await
            .unwrap();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
