//! JSON API for books, chapters and scenes, under `/api/v1`. Thin: every rule lives in the store.
//! Device authentication is added in a later part; until then this router must not be reachable
//! from the network (see `main.rs`).

use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, FromRequestParts, Path, Request, State},
    http::StatusCode,
    http::{HeaderValue, header, request::Parts},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post, put},
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    device::{self, Device, DeviceInfo},
    import::{ImportFile, ParagraphMode},
    store::{
        Book, Chapter, ExportFile, ImportSummary, SaveScene, Saved, Scene, SceneFields, SceneInfo,
        Store, StoreError,
    },
};

type Shared = Arc<Store>;
pub(crate) type ApiResult<T> = Result<T, ApiError>;

/// Largest import request (a whole manuscript as JSON).
const IMPORT_LIMIT: usize = 16 * 1024 * 1024;

pub struct ApiError(pub(crate) StoreError);

impl From<StoreError> for ApiError {
    fn from(e: StoreError) -> Self {
        ApiError(e)
    }
}

fn error_body(code: &str, message: &str) -> Value {
    json!({ "error": { "code": code, "message": message } })
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, body) = match &self.0 {
            StoreError::NotFound(m) => (StatusCode::NOT_FOUND, error_body("not_found", m)),
            StoreError::Invalid(m) => (StatusCode::UNPROCESSABLE_ENTITY, error_body("invalid", m)),
            StoreError::Dirty => (
                StatusCode::CONFLICT,
                error_body(
                    "repository_dirty",
                    "the book repository has uncommitted changes; the owner must look at it",
                ),
            ),
            StoreError::ImportRejected(issues) => {
                let mut body = error_body(
                    "import_rejected",
                    "the files were not understood; nothing was imported",
                );
                body["issues"] = issues
                    .iter()
                    .map(|i| json!({ "file": i.file, "line": i.line, "message": i.message }))
                    .collect();
                (StatusCode::UNPROCESSABLE_ENTITY, body)
            }
            StoreError::Exists(m) => (StatusCode::CONFLICT, error_body("already_exists", m)),
            StoreError::InUse(m) => (StatusCode::CONFLICT, error_body("in_use", m)),
            StoreError::Transition(m) => {
                (StatusCode::CONFLICT, error_body("invalid_transition", m))
            }
            StoreError::Unauthorized => return unauthorized(),
            StoreError::Conflict(current) => {
                let mut body =
                    error_body("revision_conflict", "the scene changed since it was read");
                body["current"] = serde_json::to_value(current).unwrap_or(Value::Null);
                (StatusCode::CONFLICT, body)
            }
            StoreError::Internal(m) => {
                eprintln!("pero-server: {m}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    error_body("internal", "internal error"),
                )
            }
        };
        (status, Json(body)).into_response()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NewBook {
    title: String,
    language: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportRequest {
    title: String,
    language: String,
    #[serde(default)]
    mode: ParagraphMode,
    files: Vec<ImportFile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Title {
    title: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NewChapter {
    title: String,
    position: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NewScene {
    #[serde(default)]
    title: String,
    language: Option<String>,
    pov: Option<String>,
    date: Option<String>,
    location: Option<String>,
    position: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Order {
    ids: Vec<String>,
}

async fn list_books(State(s): State<Shared>) -> ApiResult<Json<Vec<Book>>> {
    Ok(Json(s.list_books().await?))
}

async fn create_book(
    State(s): State<Shared>,
    AuthDevice(device): AuthDevice,
    Json(b): Json<NewBook>,
) -> ApiResult<(StatusCode, Json<Book>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.create_book(&b.title, &b.language, &device.name).await?),
    ))
}

async fn get_book(State(s): State<Shared>, Path(id): Path<String>) -> ApiResult<Json<Book>> {
    Ok(Json(s.get_book(&id).await?))
}

async fn rename_book(
    State(s): State<Shared>,
    AuthDevice(device): AuthDevice,
    Path(id): Path<String>,
    Json(t): Json<Title>,
) -> ApiResult<Json<Book>> {
    Ok(Json(
        s.update_book_title(&id, &t.title, &device.name).await?,
    ))
}

async fn list_chapters(
    State(s): State<Shared>,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<Chapter>>> {
    Ok(Json(s.list_chapters(&id).await?))
}

async fn create_chapter(
    State(s): State<Shared>,
    AuthDevice(device): AuthDevice,
    Path(id): Path<String>,
    Json(c): Json<NewChapter>,
) -> ApiResult<(StatusCode, Json<Chapter>)> {
    Ok((
        StatusCode::CREATED,
        Json(
            s.create_chapter(&id, &c.title, c.position, &device.name)
                .await?,
        ),
    ))
}

async fn reorder_chapters(
    State(s): State<Shared>,
    AuthDevice(device): AuthDevice,
    Path(id): Path<String>,
    Json(o): Json<Order>,
) -> ApiResult<Json<Vec<Chapter>>> {
    Ok(Json(s.reorder_chapters(&id, &o.ids, &device.name).await?))
}

async fn import_book(
    State(s): State<Shared>,
    AuthDevice(device): AuthDevice,
    Json(r): Json<ImportRequest>,
) -> ApiResult<(StatusCode, Json<ImportSummary>)> {
    let summary = s
        .import_book(&r.title, &r.language, r.mode, &r.files, &device.name)
        .await?;
    Ok((StatusCode::CREATED, Json(summary)))
}

async fn export_book(State(s): State<Shared>, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    let files: Vec<ExportFile> = s.export_book(&id).await?;
    Ok(Json(json!({ "files": files })))
}

async fn reindex(State(s): State<Shared>, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    let report = s.reindex_book(&id).await?;
    Ok(Json(json!({ "unmarked_paragraphs": report.unmarked })))
}

async fn get_chapter(State(s): State<Shared>, Path(id): Path<String>) -> ApiResult<Json<Chapter>> {
    Ok(Json(s.get_chapter(&id).await?))
}

async fn rename_chapter(
    State(s): State<Shared>,
    AuthDevice(device): AuthDevice,
    Path(id): Path<String>,
    Json(t): Json<Title>,
) -> ApiResult<Json<Chapter>> {
    Ok(Json(
        s.update_chapter_title(&id, &t.title, &device.name).await?,
    ))
}

async fn list_scenes(
    State(s): State<Shared>,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<SceneInfo>>> {
    Ok(Json(s.list_scenes(&id).await?))
}

async fn create_scene(
    State(s): State<Shared>,
    AuthDevice(device): AuthDevice,
    Path(id): Path<String>,
    Json(n): Json<NewScene>,
) -> ApiResult<(StatusCode, Json<Scene>)> {
    let fields = SceneFields {
        title: n.title,
        language: n.language,
        pov: n.pov,
        date: n.date,
        location: n.location,
    };
    Ok((
        StatusCode::CREATED,
        Json(
            s.create_scene(&id, &fields, n.position, &device.name)
                .await?,
        ),
    ))
}

async fn reorder_scenes(
    State(s): State<Shared>,
    AuthDevice(device): AuthDevice,
    Path(id): Path<String>,
    Json(o): Json<Order>,
) -> ApiResult<Json<Vec<SceneInfo>>> {
    Ok(Json(s.reorder_scenes(&id, &o.ids, &device.name).await?))
}

async fn get_scene(State(s): State<Shared>, Path(id): Path<String>) -> ApiResult<Json<Scene>> {
    Ok(Json(s.read_scene(&id).await?))
}

async fn save_scene(
    State(s): State<Shared>,
    AuthDevice(device): AuthDevice,
    Path(id): Path<String>,
    Json(body): Json<SaveScene>,
) -> ApiResult<Json<Saved>> {
    Ok(Json(s.save_scene(&id, &body, &device.name).await?))
}

/// The device that made the request; present on every route behind `require_device`.
pub struct AuthDevice(pub Device);

impl<S: Send + Sync> FromRequestParts<S> for AuthDevice {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, ApiError> {
        parts
            .extensions
            .get::<Device>()
            .cloned()
            .map(AuthDevice)
            .ok_or(ApiError(StoreError::Unauthorized))
    }
}

/// 401 with the same body whatever was wrong (no token, wrong shape, unknown, revoked), so a caller
/// learns nothing about which tokens exist.
fn unauthorized() -> Response {
    let mut res = (
        StatusCode::UNAUTHORIZED,
        Json(error_body(
            "unauthorized",
            "a valid device token is required",
        )),
    )
        .into_response();
    res.headers_mut()
        .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
    res
}

fn bearer(req: &Request) -> Option<&str> {
    req.headers()
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(str::trim)
}

/// Every API request needs `Authorization: Bearer <token>` of a device that is not revoked. The
/// token is checked against the database on every request, so a revocation applies at once.
async fn require_device(State(store): State<Shared>, mut req: Request, next: Next) -> Response {
    let Some(token) = bearer(&req) else {
        return unauthorized();
    };
    match device::authenticate(store.pool(), token).await {
        Ok(Some(device)) => {
            req.extensions_mut().insert(device);
            next.run(req).await
        }
        Ok(None) => unauthorized(),
        Err(e) => ApiError(e).into_response(),
    }
}

async fn me(AuthDevice(device): AuthDevice) -> Json<Device> {
    Json(device)
}

async fn list_devices(State(s): State<Shared>) -> ApiResult<Json<Vec<DeviceInfo>>> {
    Ok(Json(device::list(s.pool()).await?))
}

/// Any device can revoke any device, itself included. Devices are registered only on the server
/// (`pero-server device add`), so a stolen token can lock others out but cannot mint new tokens.
async fn revoke_device(
    State(s): State<Shared>,
    Path(id): Path<String>,
) -> ApiResult<Json<DeviceInfo>> {
    if !crate::ids::is_id(&id, crate::ids::DEVICE) {
        return Err(StoreError::NotFound("device not found".into()).into());
    }
    Ok(Json(device::revoke(s.pool(), &id).await?))
}

pub fn router(store: Arc<Store>) -> Router {
    let graph = crate::graph_api::router(store.clone());
    let canon = crate::canon_api::router(store.clone());
    let guard = store.clone();
    Router::new()
        .route("/api/v1/books", get(list_books).post(create_book))
        .route(
            "/api/v1/books/import",
            post(import_book).layer(DefaultBodyLimit::max(IMPORT_LIMIT)),
        )
        .route("/api/v1/books/{id}", get(get_book).patch(rename_book))
        .route("/api/v1/books/{id}/export", get(export_book))
        .route(
            "/api/v1/books/{id}/chapters",
            get(list_chapters).post(create_chapter),
        )
        .route("/api/v1/books/{id}/chapters/order", put(reorder_chapters))
        .route("/api/v1/books/{id}/reindex", post(reindex))
        .route(
            "/api/v1/chapters/{id}",
            get(get_chapter).patch(rename_chapter),
        )
        .route(
            "/api/v1/chapters/{id}/scenes",
            get(list_scenes).post(create_scene),
        )
        .route("/api/v1/chapters/{id}/scenes/order", put(reorder_scenes))
        .route("/api/v1/scenes/{id}", get(get_scene).put(save_scene))
        .with_state(store)
        .route("/api/v1/me", get(me))
        .route("/api/v1/devices", get(list_devices))
        .route("/api/v1/devices/{id}/revoke", post(revoke_device))
        .with_state(guard.clone())
        .merge(graph)
        .merge(canon)
        .layer(middleware::from_fn_with_state(guard, require_device))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{device, testkit::Server};

    #[tokio::test]
    async fn a_book_can_be_written_and_read_through_the_api() {
        let s = Server::new("flow").await;
        let (st, book) = s
            .call(
                "POST",
                "/api/v1/books",
                Some(json!({"title": "Тихий Дон", "language": "ru"})),
            )
            .await;
        assert_eq!(st, StatusCode::CREATED, "{book}");
        let book_id = book["id"].as_str().unwrap().to_string();
        let (st, chapter) = s
            .call(
                "POST",
                &format!("/api/v1/books/{book_id}/chapters"),
                Some(json!({"title": "Глава первая"})),
            )
            .await;
        assert_eq!(st, StatusCode::CREATED, "{chapter}");
        let chapter_id = chapter["id"].as_str().unwrap().to_string();
        let (st, scene) = s
            .call(
                "POST",
                &format!("/api/v1/chapters/{chapter_id}/scenes"),
                Some(json!({"title": "Утро", "pov": "Григорий"})),
            )
            .await;
        assert_eq!(st, StatusCode::CREATED, "{scene}");
        let scene_id = scene["id"].as_str().unwrap().to_string();
        assert_eq!(scene["passages"], json!([]));

        let put = json!({
            "base_revision": scene["revision"],
            "fields": {"title": "Утро", "pov": "Григорий"},
            "passages": [{"text": "— Ну что, казак, — сказал он."}, {"text": "«Ёлочки» и ёж."}]
        });
        let (st, saved) = s
            .call(
                "PUT",
                &format!("/api/v1/scenes/{scene_id}"),
                Some(put.clone()),
            )
            .await;
        assert_eq!(st, StatusCode::OK, "{saved}");
        assert!(saved["commit"].is_string());
        let passages = saved["scene"]["passages"].as_array().unwrap();
        assert_eq!(passages.len(), 2);
        assert_eq!(passages[0]["text"], "— Ну что, казак, — сказал он.");
        assert!(passages[0]["id"].as_str().unwrap().len() == 8);

        let (st, got) = s
            .call("GET", &format!("/api/v1/scenes/{scene_id}"), None)
            .await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(got["passages"], saved["scene"]["passages"]);
        assert_eq!(got["revision"], saved["scene"]["revision"]);

        let (_, scenes) = s
            .call(
                "GET",
                &format!("/api/v1/chapters/{chapter_id}/scenes"),
                None,
            )
            .await;
        assert_eq!(scenes.as_array().unwrap().len(), 1);
        let (_, books) = s.call("GET", "/api/v1/books", None).await;
        assert_eq!(books[0]["title"], "Тихий Дон");

        // the same request again is based on an old revision: 409 with the current scene
        let (st, conflict) = s
            .call("PUT", &format!("/api/v1/scenes/{scene_id}"), Some(put))
            .await;
        assert_eq!(st, StatusCode::CONFLICT);
        assert_eq!(conflict["error"]["code"], "revision_conflict");
        assert_eq!(conflict["current"]["revision"], saved["scene"]["revision"]);
    }

    #[tokio::test]
    async fn errors_use_the_right_status_and_a_json_body() {
        let s = Server::new("errors").await;
        let (st, body) = s.call("GET", "/api/v1/books/bk_00000000", None).await;
        assert_eq!(
            (st, body["error"]["code"].as_str()),
            (StatusCode::NOT_FOUND, Some("not_found"))
        );
        let (st, body) = s
            .call(
                "POST",
                "/api/v1/books",
                Some(json!({"title": "", "language": "ru"})),
            )
            .await;
        assert_eq!(
            (st, body["error"]["code"].as_str()),
            (StatusCode::UNPROCESSABLE_ENTITY, Some("invalid"))
        );
        let (st, _) = s
            .call(
                "POST",
                "/api/v1/books",
                Some(json!({"title": "X", "language": "ru", "extra": 1})),
            )
            .await;
        assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
        let (st, _) = s.call("GET", "/api/v1/scenes/..%2F..%2Fetc", None).await;
        assert_eq!(st, StatusCode::NOT_FOUND);
        let (st, _) = s.call("DELETE", "/api/v1/books/bk_00000000", None).await;
        assert_eq!(st, StatusCode::METHOD_NOT_ALLOWED);

        let (_, book) = s
            .call(
                "POST",
                "/api/v1/books",
                Some(json!({"title": "Книга", "language": "ru"})),
            )
            .await;
        let (_, chapter) = s
            .call(
                "POST",
                &format!("/api/v1/books/{}/chapters", book["id"].as_str().unwrap()),
                Some(json!({"title": "Г"})),
            )
            .await;
        let (_, scene) = s
            .call(
                "POST",
                &format!(
                    "/api/v1/chapters/{}/scenes",
                    chapter["id"].as_str().unwrap()
                ),
                Some(json!({})),
            )
            .await;
        let sid = scene["id"].as_str().unwrap();
        let bad = json!({"base_revision": scene["revision"], "fields": {}, "passages": [{"text": "раз\n\nдва"}]});
        let (st, body) = s
            .call("PUT", &format!("/api/v1/scenes/{sid}"), Some(bad))
            .await;
        assert_eq!(
            (st, body["error"]["code"].as_str()),
            (StatusCode::UNPROCESSABLE_ENTITY, Some("invalid"))
        );
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("blank line")
        );
    }

    #[tokio::test]
    async fn a_manuscript_is_imported_and_exported_through_the_api() {
        let s = Server::new("import").await;
        let chapter = "# Глава первая\n\n## Утро\n\nНад Доном вставало солнце.\n\n* * *\n\n— Ну что, казак?\n";
        let body = json!({"title": "Тихий Дон", "language": "ru", "files": [{"name": "01.md", "content": chapter}]});
        let (st, summary) = s.call("POST", "/api/v1/books/import", Some(body)).await;
        assert_eq!(st, StatusCode::CREATED, "{summary}");
        assert_eq!(
            (
                summary["chapters"].as_i64(),
                summary["scenes"].as_i64(),
                summary["passages"].as_i64()
            ),
            (Some(1), Some(2), Some(2))
        );
        let id = summary["book"]["id"].as_str().unwrap();
        let (st, out) = s
            .call("GET", &format!("/api/v1/books/{id}/export"), None)
            .await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(out["files"][0]["content"], chapter);
        assert_eq!(out["files"][0]["name"], "001-glava-pervaya.md");

        let bad = json!({"title": "X", "language": "ru", "files": [{"name": "a.md", "content": "# Г\n\n### нельзя\n"}]});
        let (st, refused) = s.call("POST", "/api/v1/books/import", Some(bad)).await;
        assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(refused["error"]["code"], "import_rejected");
        assert_eq!(
            (
                refused["issues"][0]["file"].as_str(),
                refused["issues"][0]["line"].as_i64()
            ),
            (Some("a.md"), Some(3))
        );
        let (_, books) = s.call("GET", "/api/v1/books", None).await;
        assert_eq!(books.as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn imports_above_the_default_body_limit_are_accepted() {
        let s = Server::new("bigimport").await;
        let paragraph = format!(
            "{}\n\n",
            "Над Доном вставало солнце — холодное, чужое. ".repeat(22)
        )
        .repeat(3_000); // about 3 MB
        let body = json!({"title": "Большая", "language": "ru", "files": [{"name": "01.md", "content": format!("# Глава\n\n{paragraph}")}]});
        assert!(body.to_string().len() > 2 * 1024 * 1024);
        let (st, summary) = s.call("POST", "/api/v1/books/import", Some(body)).await;
        assert_eq!(st, StatusCode::CREATED, "{summary}");
        assert_eq!(summary["passages"], 3_000);
        let (st, _) = s
            .call(
                "POST",
                "/api/v1/books",
                Some(json!({"title": "X".repeat(3_000_000), "language": "ru"})),
            )
            .await;
        assert_eq!(
            st,
            StatusCode::PAYLOAD_TOO_LARGE,
            "other routes keep the small default limit"
        );
    }

    #[tokio::test]
    async fn titles_and_order_are_changed_through_the_api() {
        let s = Server::new("order").await;
        let (_, book) = s
            .call(
                "POST",
                "/api/v1/books",
                Some(json!({"title": "Книга", "language": "en"})),
            )
            .await;
        let bid = book["id"].as_str().unwrap();
        let (_, c1) = s
            .call(
                "POST",
                &format!("/api/v1/books/{bid}/chapters"),
                Some(json!({"title": "Один"})),
            )
            .await;
        let (_, c2) = s
            .call(
                "POST",
                &format!("/api/v1/books/{bid}/chapters"),
                Some(json!({"title": "Два", "position": 0})),
            )
            .await;
        assert_eq!(c2["position"], 0);
        let (st, ordered) = s
            .call(
                "PUT",
                &format!("/api/v1/books/{bid}/chapters/order"),
                Some(json!({"ids": [c1["id"], c2["id"]]})),
            )
            .await;
        assert_eq!(st, StatusCode::OK, "{ordered}");
        assert_eq!(ordered[0]["id"], c1["id"]);
        let (st, renamed) = s
            .call(
                "PATCH",
                &format!("/api/v1/chapters/{}", c1["id"].as_str().unwrap()),
                Some(json!({"title": "Начало"})),
            )
            .await;
        assert_eq!(
            (st, renamed["title"].as_str()),
            (StatusCode::OK, Some("Начало"))
        );
        let (st, b) = s
            .call(
                "PATCH",
                &format!("/api/v1/books/{bid}"),
                Some(json!({"title": "Новая"})),
            )
            .await;
        assert_eq!((st, b["title"].as_str()), (StatusCode::OK, Some("Новая")));
        let (st, r) = s
            .call("POST", &format!("/api/v1/books/{bid}/reindex"), None)
            .await;
        assert_eq!(
            (st, r["unmarked_paragraphs"].as_i64()),
            (StatusCode::OK, Some(0))
        );
    }

    // ---- device authentication ----

    /// One request per route group (and a few that do not exist): none may be answered without a token.
    const ROUTES: &[(&str, &str)] = &[
        ("GET", "/api/v1/books"),
        ("POST", "/api/v1/books"),
        ("POST", "/api/v1/books/import"),
        ("GET", "/api/v1/books/bk_00000000"),
        ("GET", "/api/v1/books/bk_00000000/export"),
        ("POST", "/api/v1/books/bk_00000000/reindex"),
        ("GET", "/api/v1/chapters/ch_00000000"),
        ("GET", "/api/v1/scenes/sc_00000000"),
        ("PUT", "/api/v1/scenes/sc_00000000"),
        ("GET", "/api/v1/books/bk_00000000/entities"),
        ("DELETE", "/api/v1/entities/en_00000000"),
        ("GET", "/api/v1/books/bk_00000000/relationships"),
        ("GET", "/api/v1/books/bk_00000000/events"),
        ("GET", "/api/v1/books/bk_00000000/facts"),
        ("POST", "/api/v1/facts/ft_00000000/approve"),
        ("GET", "/api/v1/passages/k7m2x9qa"),
        ("GET", "/api/v1/me"),
        ("GET", "/api/v1/devices"),
        ("POST", "/api/v1/devices/dv_00000000/revoke"),
        ("GET", "/api/v1/no-such-route"),
        ("DELETE", "/api/v1/books"),
    ];

    #[tokio::test]
    async fn no_route_answers_without_a_valid_token() {
        let srv = Server::new("auth-required").await;
        let unknown_but_well_formed = device::new_token();
        let attempts: Vec<(&str, Option<String>)> = vec![
            ("no header", None),
            ("empty bearer", Some("Bearer ".into())),
            ("bearer without token", Some("Bearer".into())),
            ("wrong scheme", Some(format!("Basic {}", srv.token))),
            (
                "scheme only in lower case",
                Some(format!("bearer {}", srv.token)),
            ),
            ("token without scheme", Some(srv.token.clone())),
            ("garbage", Some("Bearer garbage".into())),
            (
                "truncated token",
                Some(format!("Bearer {}", &srv.token[..20])),
            ),
            (
                "upper-cased token",
                Some(format!("Bearer {}", srv.token.to_uppercase())),
            ),
            (
                "well-formed but unknown",
                Some(format!("Bearer {unknown_but_well_formed}")),
            ),
            (
                "two tokens",
                Some(format!("Bearer {} {}", srv.token, srv.token)),
            ),
        ];
        let mut first_body: Option<Value> = None;
        for (method, path) in ROUTES {
            for (what, header) in &attempts {
                let body = (*method == "POST" || *method == "PUT").then(|| json!({}));
                let (st, headers, json) = srv.raw(header.clone(), method, path, body).await;
                assert_eq!(st, StatusCode::UNAUTHORIZED, "{method} {path} with {what}");
                assert_eq!(headers.get("www-authenticate").unwrap(), "Bearer");
                assert_eq!(json["error"]["code"], "unauthorized");
                // the answer is identical whatever was wrong: nothing to learn about existing tokens
                assert_eq!(
                    first_body.get_or_insert(json.clone()),
                    &json,
                    "{method} {path} with {what}"
                );
                assert!(!json.to_string().contains(&srv.token));
            }
        }
        // a large body is refused before it is read
        let big = json!({"title": "X", "language": "ru", "files": [{"name": "a.md", "content": "я".repeat(3_000_000)}]});
        let (st, _) = srv
            .call_with(None, "POST", "/api/v1/books/import", Some(big))
            .await;
        assert_eq!(st, StatusCode::UNAUTHORIZED);
        // nothing was created by any of the refused requests
        let (_, books) = srv.call("GET", "/api/v1/books", None).await;
        assert_eq!(books, json!([]));
    }

    #[tokio::test]
    async fn a_valid_token_opens_every_route_and_identifies_the_device() {
        let srv = Server::new("auth-valid").await;
        for (method, path) in ROUTES {
            if *path == "/api/v1/no-such-route" || (*method == "DELETE" && *path == "/api/v1/books")
            {
                continue;
            }
            let body = (*method == "POST" || *method == "PUT").then(|| json!({}));
            let (st, _) = srv.call(method, path, body).await;
            assert_ne!(st, StatusCode::UNAUTHORIZED, "{method} {path}");
        }
        let (st, me) = srv.call("GET", "/api/v1/me", None).await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(me["name"], crate::testkit::DEVICE_NAME);
        assert!(me["id"].as_str().unwrap().starts_with("dv_"));
        assert!(!me.to_string().contains(&srv.token));
        let (_, devices) = srv.call("GET", "/api/v1/devices", None).await;
        assert_eq!(devices.as_array().unwrap().len(), 1);
        assert!(devices[0].get("token_hash").is_none() && devices[0]["last_seen_at"].is_string());
        // unknown routes with a token are a plain 404
        assert_eq!(
            srv.call("GET", "/api/v1/no-such-route", None).await.0,
            StatusCode::NOT_FOUND
        );
    }

    #[tokio::test]
    async fn revoking_one_device_leaves_the_others_working() {
        let srv = Server::new("auth-revoke").await;
        let laptop = srv.token.clone();
        let phone = srv.add_device("phone").await;
        let tablet = srv.add_device("tablet").await;
        let status = |token: &str| {
            let t = token.to_string();
            let srv = &srv;
            async move { srv.call_with(Some(&t), "GET", "/api/v1/me", None).await.0 }
        };
        for t in [&laptop, &phone, &tablet] {
            assert_eq!(status(t).await, StatusCode::OK);
        }

        // revoked on the server (what `pero-server device revoke` does): effective at once, for the running server
        device::revoke(srv.store.pool(), "phone").await.unwrap();
        assert_eq!(status(&phone).await, StatusCode::UNAUTHORIZED);
        assert_eq!(status(&laptop).await, StatusCode::OK);
        assert_eq!(status(&tablet).await, StatusCode::OK);
        let (st, _) = srv
            .call_with(Some(&phone), "GET", "/api/v1/books", None)
            .await;
        assert_eq!(
            st,
            StatusCode::UNAUTHORIZED,
            "the data routes refuse it too"
        );

        // revoked from another device through the API
        let (_, list) = srv.call("GET", "/api/v1/devices", None).await;
        let tablet_id = list
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["name"] == "tablet")
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();
        let (st, done) = srv
            .call(
                "POST",
                &format!("/api/v1/devices/{tablet_id}/revoke"),
                Some(json!({})),
            )
            .await;
        assert_eq!(st, StatusCode::OK, "{done}");
        assert!(done["revoked_at"].is_string());
        assert_eq!(status(&tablet).await, StatusCode::UNAUTHORIZED);
        assert_eq!(status(&laptop).await, StatusCode::OK);
        let (st, again) = srv
            .call(
                "POST",
                &format!("/api/v1/devices/{tablet_id}/revoke"),
                Some(json!({})),
            )
            .await;
        assert_eq!(
            (st, again["error"]["code"].as_str()),
            (StatusCode::CONFLICT, Some("invalid_transition"))
        );
        assert_eq!(
            srv.call(
                "POST",
                "/api/v1/devices/dv_00000000/revoke",
                Some(json!({}))
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            srv.call("POST", "/api/v1/devices/not-an-id/revoke", Some(json!({})))
                .await
                .0,
            StatusCode::NOT_FOUND
        );

        // a device can revoke itself; afterwards its token is dead
        let (_, me) = srv.call("GET", "/api/v1/me", None).await;
        let (st, _) = srv
            .call(
                "POST",
                &format!("/api/v1/devices/{}/revoke", me["id"].as_str().unwrap()),
                Some(json!({})),
            )
            .await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(status(&laptop).await, StatusCode::UNAUTHORIZED);

        // a new device registered afterwards works, the revoked ones stay dead
        let fresh = srv.add_device("fresh").await;
        assert_eq!(status(&fresh).await, StatusCode::OK);
        for t in [&laptop, &phone, &tablet] {
            assert_eq!(status(t).await, StatusCode::UNAUTHORIZED);
        }
        let (_, list) = srv
            .call_with(Some(&fresh), "GET", "/api/v1/devices", None)
            .await;
        assert_eq!(
            list.as_array()
                .unwrap()
                .iter()
                .filter(|d| d["revoked_at"].is_null())
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn changes_are_attributed_to_the_device_that_made_them() {
        let srv = Server::new("auth-actor").await;
        let phone = srv.add_device("phone").await;
        let (st, book) = srv
            .call(
                "POST",
                "/api/v1/books",
                Some(json!({"title": "Книга", "language": "ru"})),
            )
            .await;
        assert_eq!(st, StatusCode::CREATED);
        let book_id = book["id"].as_str().unwrap();
        let (st, _) = srv
            .call_with(
                Some(&phone),
                "POST",
                &format!("/api/v1/books/{book_id}/chapters"),
                Some(json!({"title": "Глава"})),
            )
            .await;
        assert_eq!(st, StatusCode::CREATED);
        let repo = srv.dir_of_books().join(book_id);
        let log = crate::git::log_messages(&repo);
        assert_eq!(
            log,
            ["phone|Add chapter: Глава", "laptop|Create book: Книга"]
        );

        // facts: the author's fact and its approval record the device
        let (_, fact) = srv
            .call_with(
                Some(&phone),
                "POST",
                &format!("/api/v1/books/{book_id}/facts"),
                Some(json!({"text": "Факт автора"})),
            )
            .await;
        assert_eq!(fact["approved_by"], "phone");
        let (_, ai) = srv
            .call("POST", &format!("/api/v1/books/{book_id}/facts"), Some(json!({"text": "Гипотеза", "origin": "ai", "source_ref": "skill", "evidence": []})))
            .await;
        assert!(ai["error"].is_object(), "an AI fact needs evidence: {ai}");
    }
}
