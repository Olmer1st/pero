//! `GET /health`: the server's own status and the status of the two local services.
//! No secrets and no manuscript data. Results are cached for a few seconds so that
//! repeated requests cannot turn the endpoint into a load generator against the local services.

use crate::http_probe;
use axum::{Json, Router, extract::State, routing::get};
use serde::Serialize;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;

const PROBE_TIMEOUT: Duration = Duration::from_secs(2);
const CACHE_FOR: Duration = Duration::from_secs(3);

#[derive(Clone)]
pub struct AppState {
    inner: Arc<Inner>,
}

struct Inner {
    started: Instant,
    llm: String,
    languagetool: String,
    cache: Mutex<Option<(Instant, Report)>>,
}

#[derive(Clone, Serialize, Debug, PartialEq)]
pub struct ServiceStatus {
    pub status: &'static str,
    pub latency_ms: u64,
}

#[derive(Clone, Serialize, Debug, PartialEq)]
pub struct Services {
    pub llm: ServiceStatus,
    pub languagetool: ServiceStatus,
}

#[derive(Clone, Serialize, Debug, PartialEq)]
pub struct Report {
    /// `ok` when both local services answer, otherwise `degraded`.
    pub status: &'static str,
    pub version: &'static str,
    pub uptime_seconds: u64,
    pub services: Services,
}

impl AppState {
    pub fn new(llm: String, languagetool: String) -> AppState {
        AppState {
            inner: Arc::new(Inner {
                started: Instant::now(),
                llm,
                languagetool,
                cache: Mutex::new(None),
            }),
        }
    }
}

async fn probe(addr: &str, path: &str) -> ServiceStatus {
    let t = Instant::now();
    let up = matches!(
        http_probe::get(addr, path, PROBE_TIMEOUT).await,
        Ok((200, _))
    );
    ServiceStatus {
        status: if up { "up" } else { "down" },
        latency_ms: t.elapsed().as_millis() as u64,
    }
}

async fn report(state: &AppState) -> Report {
    let mut cache = state.inner.cache.lock().await;
    let services = match cache.as_ref() {
        Some((at, r)) if at.elapsed() < CACHE_FOR => r.services.clone(),
        _ => {
            let (llm, languagetool) = tokio::join!(
                probe(&state.inner.llm, "/health"),
                probe(&state.inner.languagetool, "/v2/languages")
            );
            Services { llm, languagetool }
        }
    };
    let status = if services.llm.status == "up" && services.languagetool.status == "up" {
        "ok"
    } else {
        "degraded"
    };
    let r = Report {
        status,
        version: env!("CARGO_PKG_VERSION"),
        uptime_seconds: state.inner.started.elapsed().as_secs(),
        services,
    };
    if cache
        .as_ref()
        .is_none_or(|(at, _)| at.elapsed() >= CACHE_FOR)
    {
        *cache = Some((Instant::now(), r.clone()));
    }
    r
}

async fn health(State(state): State<AppState>) -> Json<Report> {
    Json(report(&state).await)
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    /// A fake local service that answers every request with the given status line.
    async fn fake(status: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        tokio::spawn(async move {
            loop {
                let Ok((mut s, _)) = listener.accept().await else {
                    return;
                };
                let mut buf = [0u8; 512];
                let _ = s.read(&mut buf).await;
                let _ = s
                    .write_all(format!("HTTP/1.1 {status}\r\n\r\n").as_bytes())
                    .await;
            }
        });
        addr
    }

    #[tokio::test]
    async fn ok_when_both_services_answer() {
        let state = AppState::new(fake("200 OK").await, fake("200 OK").await);
        let r = report(&state).await;
        assert_eq!(r.status, "ok");
        assert_eq!(
            (r.services.llm.status, r.services.languagetool.status),
            ("up", "up")
        );
    }

    #[tokio::test]
    async fn degraded_when_one_service_is_down() {
        let state = AppState::new(fake("200 OK").await, "127.0.0.1:1".into());
        let r = report(&state).await;
        assert_eq!(r.status, "degraded");
        assert_eq!(
            (r.services.llm.status, r.services.languagetool.status),
            ("up", "down")
        );
    }

    #[tokio::test]
    async fn a_failing_status_counts_as_down() {
        let state = AppState::new(
            fake("500 Internal Server Error").await,
            fake("200 OK").await,
        );
        assert_eq!(report(&state).await.services.llm.status, "down");
    }

    #[tokio::test]
    async fn health_is_served_as_json_over_http() {
        let state = AppState::new(fake("200 OK").await, fake("200 OK").await);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        tokio::spawn(async move { axum::serve(listener, router(state)).await.unwrap() });
        let (code, body) = http_probe::get(&addr, "/health", Duration::from_secs(5))
            .await
            .unwrap();
        assert_eq!(code, 200);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["status"], "ok");
        assert_eq!(v["services"]["llm"]["status"], "up");
        assert!(v.get("version").is_some());
        let (code, _) = http_probe::get(&addr, "/nothing", Duration::from_secs(5))
            .await
            .unwrap();
        assert_eq!(code, 404);
    }

    #[tokio::test]
    async fn results_are_cached_briefly() {
        let state = AppState::new(fake("200 OK").await, "127.0.0.1:1".into());
        let first = report(&state).await;
        let second = report(&state).await;
        assert_eq!(first.services, second.services);
    }
}
