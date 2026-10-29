//! Serves the built web client (`client/dist`) from a root-owned folder: the page at `/` and the
//! hashed files under `/assets/`. Nothing else is served, there is no directory listing, and a
//! file name may only contain letters, digits, `.`, `-` and `_` (no path can leave the folder).
//! Client-side screens use the `#` part of the address, so no other path needs the page.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use axum::{
    Router,
    extract::{Path as UrlPath, State},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};

/// Everything the page needs comes from this server; nothing is inlined or loaded from elsewhere.
const CONTENT_SECURITY_POLICY: &str = "default-src 'none'; script-src 'self'; style-src 'self'; \
img-src 'self' data:; font-src 'self'; connect-src 'self'; base-uri 'none'; form-action 'self'; \
frame-ancestors 'none'";

type Dir = Arc<PathBuf>;

pub fn router(dir: PathBuf) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/assets/{file}", get(asset))
        .with_state(Arc::new(dir))
}

/// Refuses to start the server with a folder that has no page.
pub fn check(dir: &Path) -> Result<(), String> {
    let index = dir.join("index.html");
    if index.is_file() {
        Ok(())
    } else {
        Err(format!("web_dir {} has no index.html", dir.display()))
    }
}

fn safe_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
}

fn content_type(name: &str) -> Option<&'static str> {
    let ext = name.rsplit_once('.')?.1;
    Some(match ext {
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "html" => "text/html; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "json" => "application/json",
        _ => return None,
    })
}

fn respond(body: Vec<u8>, content_type: &'static str, cache: &'static str) -> Response {
    let mut res = (StatusCode::OK, body).into_response();
    let h = res.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    h.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(CONTENT_SECURITY_POLICY),
    );
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    h.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    h.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    res
}

async fn read(path: PathBuf) -> Option<Vec<u8>> {
    tokio::fs::read(path).await.ok()
}

async fn index(State(dir): State<Dir>) -> Response {
    match read(dir.join("index.html")).await {
        Some(body) => respond(body, "text/html; charset=utf-8", "no-store"),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn asset(State(dir): State<Dir>, UrlPath(file): UrlPath<String>) -> Response {
    let (true, Some(kind)) = (safe_name(&file), content_type(&file)) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match read(dir.join("assets").join(&file)).await {
        // File names carry a content hash, so they may be cached for good.
        Some(body) => respond(body, kind, "public, max-age=31536000, immutable"),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db, device, store::Store};
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    fn site(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pero-web-test-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("assets")).unwrap();
        std::fs::write(dir.join("index.html"), "<!doctype html><title>Pero</title>").unwrap();
        std::fs::write(dir.join("assets/app-1a2b.js"), "console.log(1)").unwrap();
        std::fs::write(dir.join("assets/app-1a2b.css"), "body{}").unwrap();
        std::fs::write(dir.join("assets/.hidden.js"), "secret").unwrap();
        std::fs::write(dir.join("assets/notes.txt"), "not a web file").unwrap();
        std::fs::write(dir.join("secret.txt"), "outside assets").unwrap();
        dir
    }

    async fn get_path(app: &Router, uri: &str) -> Response {
        app.clone()
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn serves_the_page_and_assets_with_safe_headers() {
        let dir = site("serve");
        let app = router(dir.clone());

        let page = get_path(&app, "/").await;
        assert_eq!(page.status(), StatusCode::OK);
        let h = page.headers();
        assert_eq!(h[header::CONTENT_TYPE], "text/html; charset=utf-8");
        assert_eq!(h[header::CACHE_CONTROL], "no-store");
        assert!(
            h[header::CONTENT_SECURITY_POLICY]
                .to_str()
                .unwrap()
                .contains("script-src 'self'")
        );
        assert_eq!(h[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
        assert_eq!(h[header::X_FRAME_OPTIONS], "DENY");

        let js = get_path(&app, "/assets/app-1a2b.js").await;
        assert_eq!(js.status(), StatusCode::OK);
        assert_eq!(
            js.headers()[header::CONTENT_TYPE],
            "text/javascript; charset=utf-8"
        );
        assert!(
            js.headers()[header::CACHE_CONTROL]
                .to_str()
                .unwrap()
                .contains("immutable")
        );
        let body = axum::body::to_bytes(js.into_body(), 1 << 20).await.unwrap();
        assert_eq!(&body[..], b"console.log(1)");
        assert_eq!(
            get_path(&app, "/assets/app-1a2b.css").await.headers()[header::CONTENT_TYPE],
            "text/css; charset=utf-8"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn nothing_outside_the_assets_can_be_reached() {
        let dir = site("paths");
        let app = router(dir.clone());
        for uri in [
            "/assets/..%2Fsecret.txt",
            "/assets/%2E%2E%2Fsecret.txt",
            "/assets/..%5Csecret.txt",
            "/assets/.hidden.js",
            "/assets/notes.txt",
            "/assets/missing.js",
            "/assets/",
            "/assets",
            "/assets/%00.js",
            "/secret.txt",
            "/index.html",
            "/a/b",
        ] {
            let status = get_path(&app, uri).await.status();
            assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn a_folder_without_a_page_is_refused_at_start() {
        let dir = std::env::temp_dir().join(format!("pero-web-test-{}-empty", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert!(check(&dir).unwrap_err().contains("no index.html"));
        std::fs::write(dir.join("index.html"), "x").unwrap();
        assert!(check(&dir).is_ok());
        let _ = std::fs::remove_dir_all(dir);
    }

    /// The page is open, the API behind it still is not: next to the web routes every API path
    /// (also unknown ones) answers 401 without a token.
    #[tokio::test]
    async fn the_api_stays_closed_next_to_the_page() {
        let dir = site("api");
        let pool = db::open_memory().await;
        let books =
            std::env::temp_dir().join(format!("pero-web-test-{}-books", std::process::id()));
        let store = Store::new(pool, books.clone()).unwrap();
        let (_, token) = device::add(store.pool(), "laptop").await.unwrap();
        let app = crate::compose(Router::new(), Some(dir.clone()), Some(store));

        assert_eq!(get_path(&app, "/").await.status(), StatusCode::OK);
        for uri in ["/api/v1/books", "/api/v1/nothing", "/api/v1/"] {
            assert_eq!(
                get_path(&app, uri).await.status(),
                StatusCode::UNAUTHORIZED,
                "{uri}"
            );
        }
        let with_token = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/books")
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(with_token.status(), StatusCode::OK);
        let _ = std::fs::remove_dir_all(dir);
        let _ = std::fs::remove_dir_all(books);
    }
}
