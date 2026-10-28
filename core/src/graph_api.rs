//! JSON routes for the story graph under `/api/v1`: entities, relationships, timeline events.
//! Thin: every rule lives in `graph`. Like the rest of the API it has no login yet.

use std::{collections::HashMap, sync::Arc};

use axum::{
    Json, Router,
    extract::{Path, RawQuery, State},
    http::StatusCode,
    routing::get,
};

use crate::{
    api::ApiResult,
    graph::{
        self, Entity, EntityPatch, Event, EventPatch, NewEntity, NewEvent, NewRelationship,
        Relationship, RelationshipPatch,
    },
    store::{Store, StoreError},
};

type Shared = Arc<Store>;

/// Reads `?key=value&...` for the few simple filters. Values are plain ASCII tokens (kinds, IDs,
/// `true`/`false`), so no percent-decoding is needed; anything else is refused.
fn params(raw: Option<String>, allowed: &[&str]) -> ApiResult<HashMap<String, String>> {
    let mut out = HashMap::new();
    for pair in raw.unwrap_or_default().split('&').filter(|p| !p.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        let plain = |s: &str| {
            s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        };
        if !allowed.contains(&key) {
            return Err(StoreError::Invalid(format!("unknown query parameter \"{key}\"")).into());
        }
        if !plain(value) {
            return Err(StoreError::Invalid(format!("unsupported characters in \"{key}\"")).into());
        }
        out.insert(key.to_string(), value.to_string());
    }
    Ok(out)
}

// ---- entities --------------------------------------------------------------------------------

async fn list_entities(
    State(s): State<Shared>,
    Path(book): Path<String>,
    RawQuery(q): RawQuery,
) -> ApiResult<Json<Vec<Entity>>> {
    let p = params(q, &["kind"])?;
    Ok(Json(
        graph::list_entities(s.pool(), &book, p.get("kind").map(String::as_str)).await?,
    ))
}

async fn create_entity(
    State(s): State<Shared>,
    Path(book): Path<String>,
    Json(n): Json<NewEntity>,
) -> ApiResult<(StatusCode, Json<Entity>)> {
    Ok((
        StatusCode::CREATED,
        Json(graph::create_entity(s.pool(), &book, &n).await?),
    ))
}

async fn get_entity(State(s): State<Shared>, Path(id): Path<String>) -> ApiResult<Json<Entity>> {
    Ok(Json(graph::get_entity(s.pool(), &id).await?))
}

async fn patch_entity(
    State(s): State<Shared>,
    Path(id): Path<String>,
    Json(p): Json<EntityPatch>,
) -> ApiResult<Json<Entity>> {
    Ok(Json(graph::update_entity(s.pool(), &id, &p).await?))
}

async fn delete_entity(
    State(s): State<Shared>,
    Path(id): Path<String>,
    RawQuery(q): RawQuery,
) -> ApiResult<StatusCode> {
    let p = params(q, &["cascade"])?;
    let cascade = match p.get("cascade").map(String::as_str) {
        None | Some("false") => false,
        Some("true") => true,
        Some(_) => return Err(StoreError::Invalid("cascade must be true or false".into()).into()),
    };
    graph::delete_entity(s.pool(), &id, cascade).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---- relationships ---------------------------------------------------------------------------

async fn list_relationships(
    State(s): State<Shared>,
    Path(book): Path<String>,
    RawQuery(q): RawQuery,
) -> ApiResult<Json<Vec<Relationship>>> {
    let p = params(q, &["entity"])?;
    Ok(Json(
        graph::list_relationships(s.pool(), &book, p.get("entity").map(String::as_str)).await?,
    ))
}

async fn create_relationship(
    State(s): State<Shared>,
    Path(book): Path<String>,
    Json(n): Json<NewRelationship>,
) -> ApiResult<(StatusCode, Json<Relationship>)> {
    Ok((
        StatusCode::CREATED,
        Json(graph::create_relationship(s.pool(), &book, &n).await?),
    ))
}

async fn get_relationship(
    State(s): State<Shared>,
    Path(id): Path<String>,
) -> ApiResult<Json<Relationship>> {
    Ok(Json(graph::get_relationship(s.pool(), &id).await?))
}

async fn patch_relationship(
    State(s): State<Shared>,
    Path(id): Path<String>,
    Json(p): Json<RelationshipPatch>,
) -> ApiResult<Json<Relationship>> {
    Ok(Json(graph::update_relationship(s.pool(), &id, &p).await?))
}

async fn delete_relationship(
    State(s): State<Shared>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    graph::delete_relationship(s.pool(), &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---- timeline events -------------------------------------------------------------------------

async fn list_events(
    State(s): State<Shared>,
    Path(book): Path<String>,
    RawQuery(q): RawQuery,
) -> ApiResult<Json<Vec<Event>>> {
    let p = params(q, &["entity"])?;
    Ok(Json(
        graph::list_events(s.pool(), &book, p.get("entity").map(String::as_str)).await?,
    ))
}

async fn create_event(
    State(s): State<Shared>,
    Path(book): Path<String>,
    Json(n): Json<NewEvent>,
) -> ApiResult<(StatusCode, Json<Event>)> {
    Ok((
        StatusCode::CREATED,
        Json(graph::create_event(s.pool(), &book, &n).await?),
    ))
}

async fn get_event(State(s): State<Shared>, Path(id): Path<String>) -> ApiResult<Json<Event>> {
    Ok(Json(graph::get_event(s.pool(), &id).await?))
}

async fn patch_event(
    State(s): State<Shared>,
    Path(id): Path<String>,
    Json(p): Json<EventPatch>,
) -> ApiResult<Json<Event>> {
    Ok(Json(graph::update_event(s.pool(), &id, &p).await?))
}

async fn delete_event(State(s): State<Shared>, Path(id): Path<String>) -> ApiResult<StatusCode> {
    graph::delete_event(s.pool(), &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub fn router(store: Arc<Store>) -> Router {
    Router::new()
        .route(
            "/api/v1/books/{id}/entities",
            get(list_entities).post(create_entity),
        )
        .route(
            "/api/v1/entities/{id}",
            get(get_entity).patch(patch_entity).delete(delete_entity),
        )
        .route(
            "/api/v1/books/{id}/relationships",
            get(list_relationships).post(create_relationship),
        )
        .route(
            "/api/v1/relationships/{id}",
            get(get_relationship)
                .patch(patch_relationship)
                .delete(delete_relationship),
        )
        .route(
            "/api/v1/books/{id}/events",
            get(list_events).post(create_event),
        )
        .route(
            "/api/v1/events/{id}",
            get(get_event).patch(patch_event).delete(delete_event),
        )
        .with_state(store)
}

#[cfg(test)]
mod tests {
    use crate::testkit::Server;
    use axum::http::StatusCode;
    use serde_json::{Value, json};

    const CREATED: StatusCode = StatusCode::CREATED;
    const OK: StatusCode = StatusCode::OK;
    const INVALID: StatusCode = StatusCode::UNPROCESSABLE_ENTITY;
    const MISSING: StatusCode = StatusCode::NOT_FOUND;
    const CONFLICT: StatusCode = StatusCode::CONFLICT;

    fn s(v: &Value) -> String {
        v.as_str().unwrap().to_string()
    }

    async fn book(srv: &Server, title: &str) -> String {
        let (st, b) = srv
            .call(
                "POST",
                "/api/v1/books",
                Some(json!({"title": title, "language": "ru"})),
            )
            .await;
        assert_eq!(st, CREATED, "{b}");
        s(&b["id"])
    }

    async fn entity(srv: &Server, book: &str, kind: &str, name: &str) -> String {
        let (st, e) = srv
            .call(
                "POST",
                &format!("/api/v1/books/{book}/entities"),
                Some(json!({"kind": kind, "name": name})),
            )
            .await;
        assert_eq!(st, CREATED, "{e}");
        s(&e["id"])
    }

    async fn scene(srv: &Server, book: &str) -> String {
        let (_, c) = srv
            .call(
                "POST",
                &format!("/api/v1/books/{book}/chapters"),
                Some(json!({"title": "Глава"})),
            )
            .await;
        let (st, sc) = srv
            .call(
                "POST",
                &format!("/api/v1/chapters/{}/scenes", s(&c["id"])),
                Some(json!({"title": "Сцена"})),
            )
            .await;
        assert_eq!(st, CREATED, "{sc}");
        s(&sc["id"])
    }

    /// Expects a 422 `invalid` whose message mentions `needle`.
    async fn rejects(srv: &Server, method: &str, uri: &str, body: Value, needle: &str) {
        let (st, e) = srv.call(method, uri, Some(body.clone())).await;
        assert_eq!(st, INVALID, "{body} -> {e}");
        assert_eq!(e["error"]["code"], "invalid", "{body}");
        let msg = e["error"]["message"].as_str().unwrap_or_default();
        assert!(
            msg.contains(needle),
            "{body}: expected {needle:?} in {msg:?}"
        );
    }

    // ---- entities ----

    #[tokio::test]
    async fn entities_can_be_created_read_listed_changed_and_deleted() {
        let srv = Server::new("entities").await;
        let b = book(&srv, "Тихий Дон").await;
        let base = format!("/api/v1/books/{b}/entities");
        let (st, g) = srv
            .call(
                "POST",
                &base,
                Some(json!({"kind": "character", "name": "Григорий Мелехов", "description": "Казак.\nМладший сын.", "aliases": ["Гришка", "Мелехов"]})),
            )
            .await;
        assert_eq!(st, CREATED, "{g}");
        assert!(s(&g["id"]).starts_with("en_"));
        assert_eq!(
            (g["kind"].as_str(), g["name"].as_str()),
            (Some("character"), Some("Григорий Мелехов"))
        );
        assert_eq!(g["aliases"], json!(["Гришка", "Мелехов"]));
        assert_eq!(g["description"], "Казак.\nМладший сын.");
        let id = s(&g["id"]);

        let (st, got) = srv
            .call("GET", &format!("/api/v1/entities/{id}"), None)
            .await;
        assert_eq!((st, &got), (OK, &g));

        // listing: by kind, then by name ignoring case, Cyrillic included
        for name in ["борис", "Анна", "Вера"] {
            entity(&srv, &b, "character", name).await;
        }
        entity(&srv, &b, "place", "Вёшенская").await;
        entity(&srv, &b, "organization", "Полк").await;
        let (st, all) = srv.call("GET", &base, None).await;
        assert_eq!(st, OK);
        let names: Vec<&str> = all
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["name"].as_str().unwrap())
            .collect();
        assert_eq!(
            names,
            [
                "Анна",
                "борис",
                "Вера",
                "Григорий Мелехов",
                "Полк",
                "Вёшенская"
            ]
        );
        let (_, places) = srv.call("GET", &format!("{base}?kind=place"), None).await;
        assert_eq!(places.as_array().unwrap().len(), 1);
        assert_eq!(places[0]["name"], "Вёшенская");

        // changing: only the given fields; aliases are replaced as a whole
        let (st, p) = srv
            .call(
                "PATCH",
                &format!("/api/v1/entities/{id}"),
                Some(json!({"name": "Григорий Пантелеевич"})),
            )
            .await;
        assert_eq!(st, OK, "{p}");
        assert_eq!(
            (p["name"].as_str(), p["aliases"].clone(), p["kind"].as_str()),
            (
                Some("Григорий Пантелеевич"),
                json!(["Гришка", "Мелехов"]),
                Some("character")
            )
        );
        let (_, p) = srv
            .call(
                "PATCH",
                &format!("/api/v1/entities/{id}"),
                Some(json!({"aliases": ["Гриша"], "description": ""})),
            )
            .await;
        assert_eq!(
            (p["aliases"].clone(), p["description"].as_str()),
            (json!(["Гриша"]), Some(""))
        );
        let (_, p) = srv
            .call(
                "PATCH",
                &format!("/api/v1/entities/{id}"),
                Some(json!({"aliases": []})),
            )
            .await;
        assert_eq!(p["aliases"], json!([]));
        assert!(p["updated_at"].as_str().unwrap() >= p["created_at"].as_str().unwrap());

        // deleting
        let (st, _) = srv
            .call("DELETE", &format!("/api/v1/entities/{id}"), None)
            .await;
        assert_eq!(st, StatusCode::NO_CONTENT);
        assert_eq!(
            srv.call("GET", &format!("/api/v1/entities/{id}"), None)
                .await
                .0,
            MISSING
        );
        assert_eq!(
            srv.call("DELETE", &format!("/api/v1/entities/{id}"), None)
                .await
                .0,
            MISSING
        );
        let (n,): (i64,) =
            sqlx::query_as("SELECT count(*) FROM entity_aliases WHERE entity_id = ?")
                .bind(&id)
                .fetch_one(srv.store.pool())
                .await
                .unwrap();
        assert_eq!(n, 0, "aliases go with the entity");
    }

    #[tokio::test]
    async fn invalid_entities_are_refused_with_a_reason() {
        let srv = Server::new("entity-errors").await;
        let b = book(&srv, "Книга").await;
        let base = format!("/api/v1/books/{b}/entities");
        let id = entity(&srv, &b, "character", "Анна").await;
        let long_name = "я".repeat(301);
        let many: Vec<String> = (0..51).map(|i| format!("псевдоним {i}")).collect();
        let cases: Vec<(Value, &str)> = vec![
            (json!({"kind": "dragon", "name": "Х"}), "kind"),
            (json!({"kind": "character", "name": ""}), "name"),
            (json!({"kind": "character", "name": "   "}), "name"),
            (json!({"kind": "character", "name": long_name}), "name"),
            (json!({"kind": "character", "name": "А\nБ"}), "name"),
            (
                json!({"kind": "character", "name": "Анна", "aliases": ["Анна"]}),
                "alias",
            ),
            (
                json!({"kind": "character", "name": "Анна", "aliases": ["Аня", "Аня"]}),
                "different",
            ),
            (
                json!({"kind": "character", "name": "Анна", "aliases": [""]}),
                "alias",
            ),
            (
                json!({"kind": "character", "name": "Анна", "aliases": many}),
                "aliases",
            ),
            (
                json!({"kind": "character", "name": "Анна", "description": "я".repeat(5001)}),
                "description",
            ),
            (
                json!({"kind": "character", "name": "Анна", "description": "a\u{0}b"}),
                "control",
            ),
        ];
        for (body, needle) in cases {
            rejects(&srv, "POST", &base, body, needle).await;
        }
        // unknown or immutable fields, wrong types, empty changes
        assert_eq!(
            srv.call(
                "POST",
                &base,
                Some(json!({"kind": "character", "name": "А", "colour": "red"}))
            )
            .await
            .0,
            INVALID
        );
        assert_eq!(
            srv.call("POST", &base, Some(json!({"name": "А"}))).await.0,
            INVALID
        );
        assert_eq!(
            srv.call("POST", &base, Some(json!({"kind": "character", "name": 5})))
                .await
                .0,
            INVALID
        );
        let one = format!("/api/v1/entities/{id}");
        assert_eq!(
            srv.call("PATCH", &one, Some(json!({"kind": "place"})))
                .await
                .0,
            INVALID,
            "the kind cannot change"
        );
        rejects(&srv, "PATCH", &one, json!({}), "nothing to change").await;
        rejects(&srv, "PATCH", &one, json!({"name": ""}), "name").await;
        rejects(&srv, "PATCH", &one, json!({"aliases": ["Анна"]}), "alias").await;
        // unknown things
        let ghost = "en_00000000";
        assert_eq!(
            srv.call("GET", &format!("/api/v1/entities/{ghost}"), None)
                .await
                .0,
            MISSING
        );
        assert_eq!(
            srv.call(
                "PATCH",
                &format!("/api/v1/entities/{ghost}"),
                Some(json!({"name": "Х"}))
            )
            .await
            .0,
            MISSING
        );
        assert_eq!(
            srv.call("GET", "/api/v1/entities/not-an-id", None).await.0,
            MISSING
        );
        assert_eq!(
            srv.call("GET", "/api/v1/entities/..%2Fetc", None).await.0,
            MISSING
        );
        assert_eq!(
            srv.call("GET", "/api/v1/books/bk_00000000/entities", None)
                .await
                .0,
            MISSING
        );
        assert_eq!(
            srv.call(
                "POST",
                "/api/v1/books/bk_00000000/entities",
                Some(json!({"kind": "place", "name": "Х"}))
            )
            .await
            .0,
            MISSING
        );
        // query strings
        assert_eq!(
            srv.call("GET", &format!("{base}?kind=dragon"), None)
                .await
                .0,
            INVALID
        );
        assert_eq!(
            srv.call("GET", &format!("{base}?colour=red"), None).await.0,
            INVALID
        );
        assert_eq!(
            srv.call("GET", &format!("{base}?kind=%D0%B6"), None)
                .await
                .0,
            INVALID
        );
        assert_eq!(
            srv.call("DELETE", &format!("{one}?cascade=maybe"), None)
                .await
                .0,
            INVALID
        );
        assert_eq!(
            srv.call("GET", &one, None).await.0,
            OK,
            "nothing was changed or deleted by the refused calls"
        );
    }

    #[tokio::test]
    async fn names_and_descriptions_keep_their_exact_characters() {
        let srv = Server::new("cyrillic").await;
        let b = book(&srv, "Книга").await;
        let name = "Ёжик «Тихий» — друг\u{a0}Ё";
        let (_, e) = srv
            .call(
                "POST",
                &format!("/api/v1/books/{b}/entities"),
                Some(json!({"kind": "character", "name": name, "description": "„Лапки“ и… многоточие", "aliases": ["и\u{306}"]})),
            )
            .await;
        assert_eq!(e["name"].as_str().unwrap().as_bytes(), name.as_bytes());
        assert_eq!(e["description"], "„Лапки“ и… многоточие");
        assert_eq!(
            e["aliases"][0].as_str().unwrap(),
            "и\u{306}",
            "no Unicode normalisation"
        );
    }

    #[tokio::test]
    async fn deleting_an_entity_that_is_used_needs_cascade_and_never_removes_canon() {
        let srv = Server::new("entity-delete").await;
        let b = book(&srv, "Книга").await;
        let hero = entity(&srv, &b, "character", "Григорий").await;
        let friend = entity(&srv, &b, "character", "Аксинья").await;
        let town = entity(&srv, &b, "place", "Вёшенская").await;
        let (_, rel) = srv
            .call(
                "POST",
                &format!("/api/v1/books/{b}/relationships"),
                Some(json!({"from_entity_id": hero, "to_entity_id": friend, "kind": "любит"})),
            )
            .await;
        let (_, ev) = srv
            .call("POST", &format!("/api/v1/books/{b}/events"), Some(json!({"title": "Встреча", "place_id": town, "participants": [{"entity_id": hero, "role": "гость"}, {"entity_id": friend}]})))
            .await;

        // refused while used
        let (st, e) = srv
            .call("DELETE", &format!("/api/v1/entities/{hero}"), None)
            .await;
        assert_eq!(
            (st, e["error"]["code"].as_str()),
            (CONFLICT, Some("in_use"))
        );
        let (st, e) = srv
            .call("DELETE", &format!("/api/v1/entities/{town}"), None)
            .await;
        assert_eq!(
            (st, e["error"]["code"].as_str()),
            (CONFLICT, Some("in_use"))
        );
        assert_eq!(
            srv.call("GET", &format!("/api/v1/entities/{hero}"), None)
                .await
                .0,
            OK
        );

        // a canon fact about the entity blocks even a cascade
        sqlx::query("INSERT INTO facts (id, book_id, entity_id, text, status, origin) VALUES ('ft_00000001', ?, ?, 'Родился на хуторе', 'ai_hypothesis', 'ai')")
            .bind(&b)
            .bind(&friend)
            .execute(srv.store.pool())
            .await
            .unwrap();
        let (st, e) = srv
            .call(
                "DELETE",
                &format!("/api/v1/entities/{friend}?cascade=true"),
                None,
            )
            .await;
        assert_eq!(
            (st, e["error"]["code"].as_str()),
            (CONFLICT, Some("in_use"))
        );
        assert_eq!(
            srv.call(
                "GET",
                &format!("/api/v1/relationships/{}", s(&rel["id"])),
                None
            )
            .await
            .0,
            OK,
            "a refused cascade removes nothing"
        );

        // cascade removes the relationships and participations and clears place references
        assert_eq!(
            srv.call(
                "DELETE",
                &format!("/api/v1/entities/{hero}?cascade=true"),
                None
            )
            .await
            .0,
            StatusCode::NO_CONTENT
        );
        assert_eq!(
            srv.call(
                "GET",
                &format!("/api/v1/relationships/{}", s(&rel["id"])),
                None
            )
            .await
            .0,
            MISSING
        );
        let (_, e) = srv
            .call("GET", &format!("/api/v1/events/{}", s(&ev["id"])), None)
            .await;
        assert_eq!(e["participants"].as_array().unwrap().len(), 1);
        assert_eq!(
            srv.call(
                "DELETE",
                &format!("/api/v1/entities/{town}?cascade=true"),
                None
            )
            .await
            .0,
            StatusCode::NO_CONTENT
        );
        let (_, e) = srv
            .call("GET", &format!("/api/v1/events/{}", s(&ev["id"])), None)
            .await;
        assert_eq!(e["place_id"], Value::Null);
    }

    // ---- relationships ----

    #[tokio::test]
    async fn relationships_can_be_created_read_listed_changed_and_deleted() {
        let srv = Server::new("relationships").await;
        let b = book(&srv, "Книга").await;
        let a = entity(&srv, &b, "character", "Анна").await;
        let v = entity(&srv, &b, "character", "Вера").await;
        let o = entity(&srv, &b, "organization", "Полк").await;
        let base = format!("/api/v1/books/{b}/relationships");
        let (st, r) = srv
            .call("POST", &base, Some(json!({"from_entity_id": a, "to_entity_id": v, "kind": "сестра", "description": "старшая"})))
            .await;
        assert_eq!(st, CREATED, "{r}");
        assert!(s(&r["id"]).starts_with("rl_"));
        assert_eq!(
            (r["directed"].as_bool(), r["description"].as_str()),
            (Some(true), Some("старшая"))
        );
        let id = s(&r["id"]);
        let (_, got) = srv
            .call("GET", &format!("/api/v1/relationships/{id}"), None)
            .await;
        assert_eq!(got, r);

        let (_, r2) = srv
            .call(
                "POST",
                &base,
                Some(json!({"from_entity_id": v, "to_entity_id": o, "kind": "служит в"})),
            )
            .await;
        let (_, all) = srv.call("GET", &base, None).await;
        assert_eq!(all.as_array().unwrap().len(), 2);
        let (_, of_a) = srv.call("GET", &format!("{base}?entity={a}"), None).await;
        assert_eq!(of_a.as_array().unwrap().len(), 1);
        let (_, of_v) = srv.call("GET", &format!("{base}?entity={v}"), None).await;
        assert_eq!(
            of_v.as_array().unwrap().len(),
            2,
            "an entity is found at either end"
        );

        let (st, p) = srv
            .call(
                "PATCH",
                &format!("/api/v1/relationships/{id}"),
                Some(json!({"kind": "сестра по матери", "directed": false})),
            )
            .await;
        assert_eq!(st, OK, "{p}");
        assert_eq!(
            (
                p["kind"].as_str(),
                p["directed"].as_bool(),
                p["description"].as_str()
            ),
            (Some("сестра по матери"), Some(false), Some("старшая"))
        );
        assert_eq!(p["from_entity_id"], r["from_entity_id"]);

        assert_eq!(
            srv.call("DELETE", &format!("/api/v1/relationships/{id}"), None)
                .await
                .0,
            StatusCode::NO_CONTENT
        );
        assert_eq!(
            srv.call("GET", &format!("/api/v1/relationships/{id}"), None)
                .await
                .0,
            MISSING
        );
        assert_eq!(
            srv.call("DELETE", &format!("/api/v1/relationships/{id}"), None)
                .await
                .0,
            MISSING
        );
        assert_eq!(
            srv.call(
                "GET",
                &format!("/api/v1/relationships/{}", s(&r2["id"])),
                None
            )
            .await
            .0,
            OK
        );
    }

    #[tokio::test]
    async fn invalid_relationships_are_refused() {
        let srv = Server::new("relationship-errors").await;
        let b = book(&srv, "Книга").await;
        let other = book(&srv, "Другая").await;
        let a = entity(&srv, &b, "character", "Анна").await;
        let v = entity(&srv, &b, "character", "Вера").await;
        let stranger = entity(&srv, &other, "character", "Чужая").await;
        let base = format!("/api/v1/books/{b}/relationships");
        rejects(
            &srv,
            "POST",
            &base,
            json!({"from_entity_id": a, "to_entity_id": a, "kind": "сама себе"}),
            "two different",
        )
        .await;
        rejects(
            &srv,
            "POST",
            &base,
            json!({"from_entity_id": a, "to_entity_id": stranger, "kind": "знает"}),
            "to_entity_id",
        )
        .await;
        rejects(
            &srv,
            "POST",
            &base,
            json!({"from_entity_id": "en_00000000", "to_entity_id": v, "kind": "знает"}),
            "from_entity_id",
        )
        .await;
        rejects(
            &srv,
            "POST",
            &base,
            json!({"from_entity_id": "garbage", "to_entity_id": v, "kind": "знает"}),
            "from_entity_id",
        )
        .await;
        rejects(
            &srv,
            "POST",
            &base,
            json!({"from_entity_id": a, "to_entity_id": v, "kind": ""}),
            "kind",
        )
        .await;
        rejects(
            &srv,
            "POST",
            &base,
            json!({"from_entity_id": a, "to_entity_id": v, "kind": "я".repeat(101)}),
            "kind",
        )
        .await;
        rejects(&srv, "POST", &base, json!({"from_entity_id": a, "to_entity_id": v, "kind": "знает", "description": "я".repeat(5001)}), "description").await;
        assert_eq!(
            srv.call(
                "POST",
                &base,
                Some(json!({"from_entity_id": a, "to_entity_id": v}))
            )
            .await
            .0,
            INVALID
        );
        assert_eq!(
            srv.call(
                "POST",
                &base,
                Some(json!({"from_entity_id": a, "to_entity_id": v, "kind": "знает", "weight": 3}))
            )
            .await
            .0,
            INVALID
        );
        assert_eq!(
            srv.call(
                "POST",
                "/api/v1/books/bk_00000000/relationships",
                Some(json!({"from_entity_id": a, "to_entity_id": v, "kind": "х"}))
            )
            .await
            .0,
            MISSING
        );
        assert_eq!(
            srv.call("GET", &format!("{base}?entity={stranger}"), None)
                .await
                .0,
            INVALID,
            "an entity of another book"
        );
        assert_eq!(
            srv.call("GET", &format!("{base}?kind=x"), None).await.0,
            INVALID
        );
        let (_, none) = srv.call("GET", &base, None).await;
        assert_eq!(none, json!([]), "nothing was created by the refused calls");
    }

    #[tokio::test]
    async fn duplicate_relationships_are_refused_but_opposite_directions_are_allowed() {
        let srv = Server::new("relationship-dupes").await;
        let b = book(&srv, "Книга").await;
        let a = entity(&srv, &b, "character", "Анна").await;
        let v = entity(&srv, &b, "character", "Вера").await;
        let base = format!("/api/v1/books/{b}/relationships");
        let make = |from: &str, to: &str, kind: &str, directed: bool| json!({"from_entity_id": from, "to_entity_id": to, "kind": kind, "directed": directed});
        let (st, first) = srv
            .call("POST", &base, Some(make(&a, &v, "любит", true)))
            .await;
        assert_eq!(st, CREATED);
        let (st, e) = srv
            .call("POST", &base, Some(make(&a, &v, "любит", true)))
            .await;
        assert_eq!(
            (st, e["error"]["code"].as_str()),
            (CONFLICT, Some("already_exists"))
        );
        // A loves B and B loves A are two different directed facts
        assert_eq!(
            srv.call("POST", &base, Some(make(&v, &a, "любит", true)))
                .await
                .0,
            CREATED
        );
        // another kind between the same pair is fine
        assert_eq!(
            srv.call("POST", &base, Some(make(&a, &v, "сестра", false)))
                .await
                .0,
            CREATED
        );
        // an undirected relationship is the same in both directions
        let (st, e) = srv
            .call("POST", &base, Some(make(&v, &a, "сестра", false)))
            .await;
        assert_eq!(
            (st, e["error"]["code"].as_str()),
            (CONFLICT, Some("already_exists"))
        );
        // changing a relationship into a duplicate is refused too
        let (_, other) = srv
            .call("POST", &base, Some(make(&a, &v, "друг", true)))
            .await;
        let (st, e) = srv
            .call(
                "PATCH",
                &format!("/api/v1/relationships/{}", s(&other["id"])),
                Some(json!({"kind": "любит"})),
            )
            .await;
        assert_eq!(
            (st, e["error"]["code"].as_str()),
            (CONFLICT, Some("already_exists"))
        );
        // the two entities of a relationship cannot be changed, and an empty change is refused
        let one = format!("/api/v1/relationships/{}", s(&first["id"]));
        assert_eq!(
            srv.call("PATCH", &one, Some(json!({"to_entity_id": a})))
                .await
                .0,
            INVALID
        );
        rejects(&srv, "PATCH", &one, json!({}), "nothing to change").await;
        rejects(&srv, "PATCH", &one, json!({"kind": ""}), "kind").await;
        assert_eq!(
            srv.call(
                "PATCH",
                "/api/v1/relationships/rl_00000000",
                Some(json!({"kind": "х"}))
            )
            .await
            .0,
            MISSING
        );
    }

    // ---- timeline events ----

    #[tokio::test]
    async fn events_can_be_created_read_listed_changed_and_deleted() {
        let srv = Server::new("events").await;
        let b = book(&srv, "Книга").await;
        let hero = entity(&srv, &b, "character", "Григорий").await;
        let friend = entity(&srv, &b, "character", "Аксинья").await;
        let town = entity(&srv, &b, "place", "Вёшенская").await;
        let sc = scene(&srv, &b).await;
        let base = format!("/api/v1/books/{b}/events");

        let (st, e1) = srv
            .call(
                "POST",
                &base,
                Some(json!({"title": "Встреча на хуторе", "description": "Первая встреча.", "when_text": "весна 1914", "scene_id": sc, "place_id": town, "participants": [{"entity_id": hero, "role": "гость"}, {"entity_id": friend}]})),
            )
            .await;
        assert_eq!(st, CREATED, "{e1}");
        assert!(s(&e1["id"]).starts_with("ev_"));
        assert_eq!(
            (e1["sort_order"].as_i64(), e1["when_text"].as_str()),
            (Some(10), Some("весна 1914"))
        );
        assert_eq!(e1["participants"].as_array().unwrap().len(), 2);
        let (_, got) = srv
            .call("GET", &format!("/api/v1/events/{}", s(&e1["id"])), None)
            .await;
        assert_eq!(got, e1);

        // new events go to the end by default; an explicit order can put one in front
        let (_, e2) = srv
            .call("POST", &base, Some(json!({"title": "Свадьба"})))
            .await;
        assert_eq!(e2["sort_order"], 20);
        let (_, e0) = srv
            .call(
                "POST",
                &base,
                Some(json!({"title": "Детство", "sort_order": -5})),
            )
            .await;
        let (_, all) = srv.call("GET", &base, None).await;
        let titles: Vec<&str> = all
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["title"].as_str().unwrap())
            .collect();
        assert_eq!(titles, ["Детство", "Встреча на хуторе", "Свадьба"]);
        let (_, mine) = srv
            .call("GET", &format!("{base}?entity={friend}"), None)
            .await;
        assert_eq!(mine.as_array().unwrap().len(), 1);
        let (_, none) = srv
            .call("GET", &format!("{base}?entity={town}"), None)
            .await;
        assert_eq!(none, json!([]), "a place is not a participant");

        // changing: absent keeps, null clears, a value sets
        let id = s(&e1["id"]);
        let one = format!("/api/v1/events/{id}");
        let (st, p) = srv.call("PATCH", &one, Some(json!({"title": "Первая встреча", "when_text": null, "place_id": null, "sort_order": 30}))).await;
        assert_eq!(st, OK, "{p}");
        assert_eq!(
            (
                p["title"].as_str(),
                p["when_text"].clone(),
                p["place_id"].clone(),
                p["sort_order"].as_i64()
            ),
            (Some("Первая встреча"), Value::Null, Value::Null, Some(30))
        );
        assert_eq!(
            (p["scene_id"].as_str(), p["description"].as_str()),
            (Some(sc.as_str()), Some("Первая встреча."))
        );
        assert_eq!(
            p["participants"].as_array().unwrap().len(),
            2,
            "participants untouched when absent"
        );
        let (_, p) = srv.call("PATCH", &one, Some(json!({"participants": [{"entity_id": hero, "role": "хозяин"}], "place_id": town, "scene_id": null}))).await;
        assert_eq!(
            (
                p["participants"].clone(),
                p["place_id"].as_str(),
                p["scene_id"].clone()
            ),
            (
                json!([{"entity_id": hero, "role": "хозяин"}]),
                Some(town.as_str()),
                Value::Null
            )
        );
        let (_, p) = srv
            .call("PATCH", &one, Some(json!({"participants": []})))
            .await;
        assert_eq!(p["participants"], json!([]));

        // deleting also removes the participations
        assert_eq!(
            srv.call("DELETE", &one, None).await.0,
            StatusCode::NO_CONTENT
        );
        assert_eq!(srv.call("GET", &one, None).await.0, MISSING);
        assert_eq!(srv.call("DELETE", &one, None).await.0, MISSING);
        let (n,): (i64,) =
            sqlx::query_as("SELECT count(*) FROM timeline_event_participants WHERE event_id = ?")
                .bind(&id)
                .fetch_one(srv.store.pool())
                .await
                .unwrap();
        assert_eq!(n, 0);
        assert_eq!(
            srv.call("GET", &format!("/api/v1/events/{}", s(&e0["id"])), None)
                .await
                .0,
            OK
        );
    }

    #[tokio::test]
    async fn invalid_events_are_refused() {
        let srv = Server::new("event-errors").await;
        let b = book(&srv, "Книга").await;
        let other = book(&srv, "Другая").await;
        let hero = entity(&srv, &b, "character", "Григорий").await;
        let town = entity(&srv, &b, "place", "Вёшенская").await;
        let stranger = entity(&srv, &other, "character", "Чужая").await;
        let far_place = entity(&srv, &other, "place", "Далёкая").await;
        let sc = scene(&srv, &b).await;
        let far_scene = scene(&srv, &other).await;
        let base = format!("/api/v1/books/{b}/events");
        let cases: Vec<(Value, &str)> = vec![
            (json!({"title": ""}), "title"),
            (json!({"title": "я".repeat(301)}), "title"),
            (
                json!({"title": "Х", "when_text": "я".repeat(301)}),
                "when_text",
            ),
            (
                json!({"title": "Х", "description": "я".repeat(5001)}),
                "description",
            ),
            (json!({"title": "Х", "place_id": hero}), "not a place"),
            (json!({"title": "Х", "place_id": far_place}), "place_id"),
            (json!({"title": "Х", "place_id": "en_00000000"}), "place_id"),
            (json!({"title": "Х", "scene_id": far_scene}), "scene_id"),
            (json!({"title": "Х", "scene_id": "sc_00000000"}), "scene_id"),
            (
                json!({"title": "Х", "participants": [{"entity_id": hero}, {"entity_id": hero}]}),
                "only once",
            ),
            (
                json!({"title": "Х", "participants": [{"entity_id": stranger}]}),
                "participants",
            ),
            (
                json!({"title": "Х", "participants": [{"entity_id": hero, "role": "я".repeat(101)}]}),
                "role",
            ),
        ];
        for (body, needle) in cases {
            rejects(&srv, "POST", &base, body, needle).await;
        }
        assert_eq!(
            srv.call(
                "POST",
                &base,
                Some(json!({"title": "Х", "sort_order": "первый"}))
            )
            .await
            .0,
            INVALID
        );
        assert_eq!(
            srv.call("POST", &base, Some(json!({"title": "Х", "mood": "грусть"})))
                .await
                .0,
            INVALID
        );
        assert_eq!(
            srv.call("POST", &base, Some(json!({"description": "без названия"})))
                .await
                .0,
            INVALID
        );
        assert_eq!(
            srv.call(
                "POST",
                "/api/v1/books/bk_00000000/events",
                Some(json!({"title": "Х"}))
            )
            .await
            .0,
            MISSING
        );
        assert_eq!(
            srv.call("GET", &format!("{base}?entity={stranger}"), None)
                .await
                .0,
            INVALID
        );
        let (_, none) = srv.call("GET", &base, None).await;
        assert_eq!(none, json!([]), "nothing was created by the refused calls");

        let (_, ev) = srv
            .call(
                "POST",
                &base,
                Some(json!({"title": "Событие", "place_id": town, "scene_id": sc})),
            )
            .await;
        let one = format!("/api/v1/events/{}", s(&ev["id"]));
        rejects(&srv, "PATCH", &one, json!({}), "nothing to change").await;
        rejects(&srv, "PATCH", &one, json!({"title": ""}), "title").await;
        rejects(
            &srv,
            "PATCH",
            &one,
            json!({"place_id": hero}),
            "not a place",
        )
        .await;
        rejects(
            &srv,
            "PATCH",
            &one,
            json!({"scene_id": far_scene}),
            "scene_id",
        )
        .await;
        rejects(
            &srv,
            "PATCH",
            &one,
            json!({"participants": [{"entity_id": stranger}]}),
            "participants",
        )
        .await;
        assert_eq!(
            srv.call("PATCH", &one, Some(json!({"book_id": other})))
                .await
                .0,
            INVALID,
            "an event cannot move to another book"
        );
        assert_eq!(
            srv.call(
                "PATCH",
                "/api/v1/events/ev_00000000",
                Some(json!({"title": "Х"}))
            )
            .await
            .0,
            MISSING
        );
        let (_, same) = srv.call("GET", &one, None).await;
        assert_eq!(
            (same["place_id"].as_str(), same["scene_id"].as_str()),
            (Some(town.as_str()), Some(sc.as_str())),
            "refused changes leave the event as it was"
        );
    }

    #[tokio::test]
    async fn events_with_equal_order_are_listed_in_a_stable_order() {
        let srv = Server::new("event-order").await;
        let b = book(&srv, "Книга").await;
        let base = format!("/api/v1/books/{b}/events");
        for t in ["А", "Б", "В"] {
            srv.call("POST", &base, Some(json!({"title": t, "sort_order": 1})))
                .await;
        }
        let (_, first) = srv.call("GET", &base, None).await;
        let (_, second) = srv.call("GET", &base, None).await;
        assert_eq!(first, second);
        assert_eq!(first.as_array().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn the_graph_of_one_book_is_invisible_to_another() {
        let srv = Server::new("isolation").await;
        let a = book(&srv, "Первая").await;
        let b = book(&srv, "Вторая").await;
        entity(&srv, &a, "character", "Анна").await;
        let (_, in_b) = srv
            .call("GET", &format!("/api/v1/books/{b}/entities"), None)
            .await;
        assert_eq!(in_b, json!([]));
        let (_, in_a) = srv
            .call("GET", &format!("/api/v1/books/{a}/entities"), None)
            .await;
        assert_eq!(in_a.as_array().unwrap().len(), 1);
    }
}
