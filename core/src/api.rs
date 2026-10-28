//! JSON API for books, chapters and scenes, under `/api/v1`. Thin: every rule lives in the store.
//! Device authentication is added in a later part; until then this router must not be reachable
//! from the network (see `main.rs`).

use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post, put},
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    import::{ImportFile, ParagraphMode},
    store::{
        Book, Chapter, ExportFile, ImportSummary, SaveScene, Saved, Scene, SceneFields, SceneInfo,
        Store, StoreError,
    },
};

type Shared = Arc<Store>;
type ApiResult<T> = Result<T, ApiError>;

/// Who made the change, as recorded in Git. Becomes the device name once devices exist.
const ACTOR: &str = "api";

/// Largest import request (a whole manuscript as JSON).
const IMPORT_LIMIT: usize = 16 * 1024 * 1024;

pub struct ApiError(StoreError);

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
    Json(b): Json<NewBook>,
) -> ApiResult<(StatusCode, Json<Book>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.create_book(&b.title, &b.language, ACTOR).await?),
    ))
}

async fn get_book(State(s): State<Shared>, Path(id): Path<String>) -> ApiResult<Json<Book>> {
    Ok(Json(s.get_book(&id).await?))
}

async fn rename_book(
    State(s): State<Shared>,
    Path(id): Path<String>,
    Json(t): Json<Title>,
) -> ApiResult<Json<Book>> {
    Ok(Json(s.update_book_title(&id, &t.title, ACTOR).await?))
}

async fn list_chapters(
    State(s): State<Shared>,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<Chapter>>> {
    Ok(Json(s.list_chapters(&id).await?))
}

async fn create_chapter(
    State(s): State<Shared>,
    Path(id): Path<String>,
    Json(c): Json<NewChapter>,
) -> ApiResult<(StatusCode, Json<Chapter>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.create_chapter(&id, &c.title, c.position, ACTOR).await?),
    ))
}

async fn reorder_chapters(
    State(s): State<Shared>,
    Path(id): Path<String>,
    Json(o): Json<Order>,
) -> ApiResult<Json<Vec<Chapter>>> {
    Ok(Json(s.reorder_chapters(&id, &o.ids, ACTOR).await?))
}

async fn import_book(
    State(s): State<Shared>,
    Json(r): Json<ImportRequest>,
) -> ApiResult<(StatusCode, Json<ImportSummary>)> {
    let summary = s
        .import_book(&r.title, &r.language, r.mode, &r.files, ACTOR)
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
    Path(id): Path<String>,
    Json(t): Json<Title>,
) -> ApiResult<Json<Chapter>> {
    Ok(Json(s.update_chapter_title(&id, &t.title, ACTOR).await?))
}

async fn list_scenes(
    State(s): State<Shared>,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<SceneInfo>>> {
    Ok(Json(s.list_scenes(&id).await?))
}

async fn create_scene(
    State(s): State<Shared>,
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
        Json(s.create_scene(&id, &fields, n.position, ACTOR).await?),
    ))
}

async fn reorder_scenes(
    State(s): State<Shared>,
    Path(id): Path<String>,
    Json(o): Json<Order>,
) -> ApiResult<Json<Vec<SceneInfo>>> {
    Ok(Json(s.reorder_scenes(&id, &o.ids, ACTOR).await?))
}

async fn get_scene(State(s): State<Shared>, Path(id): Path<String>) -> ApiResult<Json<Scene>> {
    Ok(Json(s.read_scene(&id).await?))
}

async fn save_scene(
    State(s): State<Shared>,
    Path(id): Path<String>,
    Json(body): Json<SaveScene>,
) -> ApiResult<Json<Saved>> {
    Ok(Json(s.save_scene(&id, &body, ACTOR).await?))
}

pub fn router(store: Arc<Store>) -> Router {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    struct Server {
        app: Router,
        dir: std::path::PathBuf,
    }

    impl Server {
        async fn new(tag: &str) -> Server {
            let dir =
                std::env::temp_dir().join(format!("pero-api-test-{}-{tag}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            let pool = db::open(&dir.join("pero.db")).await.unwrap();
            let store = Store::new(pool, dir.join("books")).unwrap();
            Server {
                app: router(store),
                dir,
            }
        }

        async fn call(&self, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
            let req = Request::builder()
                .method(method)
                .uri(uri)
                .header("content-type", "application/json")
                .body(body.map_or_else(Body::empty, |b| Body::from(b.to_string())))
                .unwrap();
            let res = self.app.clone().oneshot(req).await.unwrap();
            let status = res.status();
            let bytes = axum::body::to_bytes(res.into_body(), 1 << 20)
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
}
