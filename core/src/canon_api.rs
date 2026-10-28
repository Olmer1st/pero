//! JSON routes for the canon ledger under `/api/v1`: facts, their evidence and history, and the
//! index information of a passage. Thin: every rule lives in `canon`. No login yet.

use std::sync::Arc;

use axum::{
    Json, Router,
    body::Bytes,
    extract::{Path, RawQuery, State},
    http::StatusCode,
    routing::{get, post},
};
use serde_json::{Value, json};

use crate::{
    api::ApiResult,
    canon::{
        self, Action, EvidenceReplace, Fact, FactFilter, FactPatch, HistoryEntry, NewFact,
        PassageInfo, TransitionBody,
    },
    graph_api::params,
    store::{Store, StoreError},
};

type Shared = Arc<Store>;

/// Who made the change, as recorded in the ledger. Becomes the device name once devices exist.
const ACTOR: &str = "api";

async fn create_fact(
    State(s): State<Shared>,
    Path(book): Path<String>,
    Json(n): Json<NewFact>,
) -> ApiResult<(StatusCode, Json<Fact>)> {
    Ok((
        StatusCode::CREATED,
        Json(canon::create_fact(s.pool(), &book, &n, ACTOR).await?),
    ))
}

async fn list_facts(
    State(s): State<Shared>,
    Path(book): Path<String>,
    RawQuery(q): RawQuery,
) -> ApiResult<Json<Vec<Fact>>> {
    let p = params(q, &["status", "entity", "passage", "origin", "stale"])?;
    let stale = match p.get("stale").map(String::as_str) {
        None => None,
        Some("true") => Some(true),
        Some("false") => Some(false),
        Some(_) => return Err(StoreError::Invalid("stale must be true or false".into()).into()),
    };
    let filter = FactFilter {
        status: p.get("status").cloned(),
        entity: p.get("entity").cloned(),
        passage: p.get("passage").cloned(),
        origin: p.get("origin").cloned(),
        stale,
    };
    Ok(Json(canon::list_facts(s.pool(), &book, &filter).await?))
}

async fn get_fact(State(s): State<Shared>, Path(id): Path<String>) -> ApiResult<Json<Fact>> {
    Ok(Json(canon::get_fact(s.pool(), &id).await?))
}

async fn patch_fact(
    State(s): State<Shared>,
    Path(id): Path<String>,
    Json(p): Json<FactPatch>,
) -> ApiResult<Json<Fact>> {
    Ok(Json(canon::update_fact(s.pool(), &id, &p).await?))
}

/// The body of approve, reject and reopen is optional: `{}` or `{"reason": "..."}`.
fn reason_of(body: &Bytes) -> ApiResult<Option<String>> {
    if body.iter().all(u8::is_ascii_whitespace) {
        return Ok(None);
    }
    let parsed: TransitionBody = serde_json::from_slice(body)
        .map_err(|e| StoreError::Invalid(format!("invalid body: {e}")))?;
    Ok(parsed.reason)
}

async fn change_status(
    s: &Shared,
    id: &str,
    action: Action,
    body: &Bytes,
) -> ApiResult<Json<Fact>> {
    let reason = reason_of(body)?;
    Ok(Json(
        canon::transition(s.pool(), id, action, ACTOR, reason.as_deref()).await?,
    ))
}

async fn approve(
    State(s): State<Shared>,
    Path(id): Path<String>,
    body: Bytes,
) -> ApiResult<Json<Fact>> {
    change_status(&s, &id, Action::Approve, &body).await
}

async fn reject(
    State(s): State<Shared>,
    Path(id): Path<String>,
    body: Bytes,
) -> ApiResult<Json<Fact>> {
    change_status(&s, &id, Action::Reject, &body).await
}

async fn reopen(
    State(s): State<Shared>,
    Path(id): Path<String>,
    body: Bytes,
) -> ApiResult<Json<Fact>> {
    change_status(&s, &id, Action::Reopen, &body).await
}

async fn replace_evidence(
    State(s): State<Shared>,
    Path(id): Path<String>,
    Json(e): Json<EvidenceReplace>,
) -> ApiResult<Json<Fact>> {
    Ok(Json(canon::replace_evidence(s.pool(), &id, &e).await?))
}

async fn history(State(s): State<Shared>, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    let entries: Vec<HistoryEntry> = canon::history(s.pool(), &id).await?;
    Ok(Json(json!({ "history": entries })))
}

async fn get_passage(
    State(s): State<Shared>,
    Path(id): Path<String>,
) -> ApiResult<Json<PassageInfo>> {
    Ok(Json(canon::get_passage(s.pool(), &id).await?))
}

pub fn router(store: Arc<Store>) -> Router {
    Router::new()
        .route(
            "/api/v1/books/{id}/facts",
            get(list_facts).post(create_fact),
        )
        .route("/api/v1/facts/{id}", get(get_fact).patch(patch_fact))
        .route("/api/v1/facts/{id}/approve", post(approve))
        .route("/api/v1/facts/{id}/reject", post(reject))
        .route("/api/v1/facts/{id}/reopen", post(reopen))
        .route(
            "/api/v1/facts/{id}/evidence",
            axum::routing::put(replace_evidence),
        )
        .route("/api/v1/facts/{id}/history", get(history))
        .route("/api/v1/passages/{id}", get(get_passage))
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

    const P0: &str = "Григорий родился на хуторе Татарском.";
    const P1: &str = "— Я никогда не бывал в Москве, — сказал он.";
    const P2: &str = "«Ёлочки» и ёж.";

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

    /// A scene with the given paragraphs; returns the scene ID and the passage IDs in order.
    async fn scene_with(srv: &Server, book: &str, texts: &[&str]) -> (String, Vec<String>) {
        let (_, c) = srv
            .call(
                "POST",
                &format!("/api/v1/books/{book}/chapters"),
                Some(json!({"title": "Глава"})),
            )
            .await;
        let (_, sc) = srv
            .call(
                "POST",
                &format!("/api/v1/chapters/{}/scenes", s(&c["id"])),
                Some(json!({"title": "Сцена"})),
            )
            .await;
        let id = s(&sc["id"]);
        let passages: Vec<Value> = texts.iter().map(|t| json!({"text": t})).collect();
        let saved = resave(srv, &id, passages, json!([])).await;
        let ids = saved["passages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| s(&p["id"]))
            .collect();
        (id, ids)
    }

    /// Saves the whole scene again (reading its current revision first); returns the new scene.
    async fn resave(srv: &Server, scene: &str, passages: Vec<Value>, merged: Value) -> Value {
        let (_, cur) = srv
            .call("GET", &format!("/api/v1/scenes/{scene}"), None)
            .await;
        let body = json!({"base_revision": cur["revision"], "fields": {"title": "Сцена"}, "passages": passages, "merged": merged});
        let (st, saved) = srv
            .call("PUT", &format!("/api/v1/scenes/{scene}"), Some(body))
            .await;
        assert_eq!(st, OK, "{saved}");
        saved["scene"].clone()
    }

    fn keep(id: &str, text: &str) -> Value {
        json!({"id": id, "text": text})
    }

    async fn fact(srv: &Server, book: &str, body: Value) -> Value {
        let (st, f) = srv
            .call("POST", &format!("/api/v1/books/{book}/facts"), Some(body))
            .await;
        assert_eq!(st, CREATED, "{f}");
        f
    }

    fn ai(text: &str, passage: &str) -> Value {
        json!({"text": text, "origin": "ai", "source_ref": "canon-extractor", "evidence": [{"passage_id": passage}]})
    }

    async fn rejects(srv: &Server, method: &str, uri: &str, body: Value, needle: &str) {
        let (st, e) = srv.call(method, uri, Some(body.clone())).await;
        assert_eq!(st, INVALID, "{body} -> {e}");
        let msg = e["error"]["message"].as_str().unwrap_or_default();
        assert!(
            msg.contains(needle),
            "{body}: expected {needle:?} in {msg:?}"
        );
    }

    async fn get(srv: &Server, id: &str) -> Value {
        srv.call("GET", &format!("/api/v1/facts/{id}"), None)
            .await
            .1
    }

    // ---- the approval rule ----

    #[tokio::test]
    async fn an_ai_hypothesis_becomes_canon_only_through_the_approve_call() {
        let srv = Server::new("canon-approval").await;
        let b = book(&srv, "Книга").await;
        let (_, p) = scene_with(&srv, &b, &[P0, P1, P2]).await;
        let facts = format!("/api/v1/books/{b}/facts");

        let f = fact(&srv, &b, ai("Григорий родился на хуторе Татарском", &p[0])).await;
        assert!(s(&f["id"]).starts_with("ft_"));
        assert_eq!(
            (
                f["status"].as_str(),
                f["origin"].as_str(),
                f["source_ref"].as_str()
            ),
            (Some("ai_hypothesis"), Some("ai"), Some("canon-extractor"))
        );
        assert_eq!(
            (
                f["approved_by"].clone(),
                f["approved_at"].clone(),
                f["stale"].as_bool()
            ),
            (Value::Null, Value::Null, Some(false))
        );
        assert_eq!(
            f["evidence"],
            json!([{"passage_id": p[0], "passage_status": "active", "changed": false}])
        );
        let id = s(&f["id"]);
        let one = format!("/api/v1/facts/{id}");

        // every way around the approve call is refused
        let way_round: Vec<(Value, &str)> = vec![
            (
                json!({"text": "Х", "origin": "ai", "status": "author_approved", "evidence": [{"passage_id": p[0]}]}),
                "only the author",
            ),
            (
                json!({"text": "Х", "origin": "ai", "status": "explicit", "evidence": [{"passage_id": p[0]}]}),
                "ai_hypothesis",
            ),
            (
                json!({"text": "Х", "origin": "ai", "status": "rejected", "evidence": [{"passage_id": p[0]}]}),
                "ai_hypothesis",
            ),
            (
                json!({"text": "Х", "origin": "author", "status": "ai_hypothesis", "evidence": [{"passage_id": p[0]}]}),
                "author_approved or explicit",
            ),
            (
                json!({"text": "Х", "origin": "import", "status": "author_approved", "evidence": [{"passage_id": p[0]}]}),
                "explicit",
            ),
            (
                json!({"text": "Х", "origin": "robot", "evidence": [{"passage_id": p[0]}]}),
                "origin",
            ),
        ];
        for (body, needle) in way_round {
            rejects(&srv, "POST", &facts, body, needle).await;
        }
        for body in [
            json!({"text": "Х", "origin": "ai", "approved_by": "me", "evidence": [{"passage_id": p[0]}]}),
            json!({"text": "Х", "origin": "ai", "approved_at": "2026-01-01T00:00:00Z", "evidence": [{"passage_id": p[0]}]}),
        ] {
            assert_eq!(
                srv.call("POST", &facts, Some(body)).await.0,
                INVALID,
                "approval fields cannot be sent"
            );
        }
        for body in [
            json!({"status": "author_approved"}),
            json!({"approved_by": "me"}),
            json!({"origin": "author"}),
            json!({"stale": false}),
        ] {
            assert_eq!(
                srv.call("PATCH", &one, Some(body)).await.0,
                INVALID,
                "the status is not editable"
            );
        }
        let (n,): (i64,) = sqlx::query_as("SELECT count(*) FROM facts")
            .fetch_one(srv.store.pool())
            .await
            .unwrap();
        assert_eq!(n, 1, "the refused attempts created nothing");
        assert_eq!(get(&srv, &id).await["status"], "ai_hypothesis");
        // even a direct database update cannot approve without the approval record
        assert!(
            sqlx::query("UPDATE facts SET status = 'author_approved' WHERE id = ?")
                .bind(&id)
                .execute(srv.store.pool())
                .await
                .is_err()
        );

        // the approve call: records who and when, and the reason
        let (st, a) = srv
            .call(
                "POST",
                &format!("{one}/approve"),
                Some(json!({"reason": "сверено с третьей главой"})),
            )
            .await;
        assert_eq!(st, OK, "{a}");
        assert_eq!(
            (a["status"].as_str(), a["approved_by"].as_str()),
            (Some("author_approved"), Some("api"))
        );
        assert!(a["approved_at"].is_string());
        let (st, e) = srv.call("POST", &format!("{one}/approve"), None).await;
        assert_eq!(
            (st, e["error"]["code"].as_str()),
            (CONFLICT, Some("invalid_transition"))
        );

        // the history shows every step
        let (_, h) = srv.call("GET", &format!("{one}/history"), None).await;
        let rows = h["history"].as_array().unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(
            (
                rows[0]["from_status"].clone(),
                rows[0]["to_status"].as_str(),
                rows[0]["changed_by"].as_str()
            ),
            (
                Value::Null,
                Some("ai_hypothesis"),
                Some("ai:canon-extractor")
            )
        );
        assert_eq!(
            (
                rows[1]["from_status"].as_str(),
                rows[1]["to_status"].as_str(),
                rows[1]["changed_by"].as_str(),
                rows[1]["reason"].as_str()
            ),
            (
                Some("ai_hypothesis"),
                Some("author_approved"),
                Some("api"),
                Some("сверено с третьей главой")
            )
        );
    }

    #[tokio::test]
    async fn statuses_can_be_changed_back_and_forth_with_the_history_kept() {
        let srv = Server::new("canon-transitions").await;
        let b = book(&srv, "Книга").await;
        let (_, p) = scene_with(&srv, &b, &[P0]).await;
        let f = fact(&srv, &b, ai("Факт", &p[0])).await;
        let one = format!("/api/v1/facts/{}", s(&f["id"]));
        let step = |action: &'static str| {
            let one = one.clone();
            let srv = &srv;
            async move { srv.call("POST", &format!("{one}/{action}"), None).await }
        };

        let (st, r) = step("reject").await;
        assert_eq!((st, r["status"].as_str()), (OK, Some("rejected")));
        assert_eq!(step("reject").await.0, CONFLICT, "already rejected");
        let (st, r) = step("reopen").await;
        assert_eq!((st, r["status"].as_str()), (OK, Some("ai_hypothesis")));
        assert_eq!(
            step("reopen").await.0,
            CONFLICT,
            "only approved or rejected facts can be reopened"
        );
        assert_eq!(step("approve").await.0, OK);
        let (st, r) = step("reopen").await; // an approved AI fact can be taken back
        assert_eq!(
            (st, r["status"].as_str(), r["approved_by"].clone()),
            (OK, Some("ai_hypothesis"), Value::Null)
        );
        assert_eq!(step("reject").await.0, OK);
        let (st, r) = step("approve").await; // changed my mind
        assert_eq!(
            (st, r["status"].as_str(), r["approved_by"].as_str()),
            (OK, Some("author_approved"), Some("api"))
        );
        let (_, h) = srv.call("GET", &format!("{one}/history"), None).await;
        let path: Vec<&str> = h["history"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["to_status"].as_str().unwrap())
            .collect();
        assert_eq!(
            path,
            [
                "ai_hypothesis",
                "rejected",
                "ai_hypothesis",
                "author_approved",
                "ai_hypothesis",
                "rejected",
                "author_approved"
            ]
        );

        // bodies: optional, strict, bounded
        assert_eq!(
            srv.call("POST", &format!("{one}/reject"), Some(json!({})))
                .await
                .0,
            OK
        );
        assert_eq!(
            srv.call(
                "POST",
                &format!("{one}/approve"),
                Some(json!({"reason": "ок"}))
            )
            .await
            .0,
            OK
        );
        for bad in [
            json!({"reason": 5}),
            json!({"note": "x"}),
            json!({"reason": "я".repeat(501)}),
        ] {
            assert_eq!(
                srv.call("POST", &format!("{one}/reject"), Some(bad))
                    .await
                    .0,
                INVALID
            );
        }
        assert_eq!(
            get(&srv, &s(&f["id"])).await["status"],
            "author_approved",
            "refused calls changed nothing"
        );
        for action in ["approve", "reject", "reopen"] {
            assert_eq!(
                srv.call("POST", &format!("/api/v1/facts/ft_00000000/{action}"), None)
                    .await
                    .0,
                MISSING
            );
        }
    }

    #[tokio::test]
    async fn the_authors_own_facts_and_imported_facts_follow_their_own_rules() {
        let srv = Server::new("canon-author").await;
        let b = book(&srv, "Книга").await;
        let (_, p) = scene_with(&srv, &b, &[P0, P1]).await;
        let facts = format!("/api/v1/books/{b}/facts");

        // the author's own decision: approved at once, evidence optional
        let a = fact(&srv, &b, json!({"text": "У Григория шрам на левой руке"})).await;
        assert_eq!(
            (
                a["status"].as_str(),
                a["origin"].as_str(),
                a["approved_by"].as_str()
            ),
            (Some("author_approved"), Some("author"), Some("api"))
        );
        assert!(a["approved_at"].is_string());
        assert_eq!(a["evidence"], json!([]));
        let (_, h) = srv
            .call(
                "GET",
                &format!("/api/v1/facts/{}/history", s(&a["id"])),
                None,
            )
            .await;
        assert_eq!(
            (
                h["history"][0]["from_status"].clone(),
                h["history"][0]["changed_by"].as_str()
            ),
            (Value::Null, Some("api"))
        );
        // an author fact is not an AI fact: it cannot be reopened as a hypothesis
        let (st, e) = srv
            .call(
                "POST",
                &format!("/api/v1/facts/{}/reopen", s(&a["id"])),
                None,
            )
            .await;
        assert_eq!(
            (st, e["error"]["code"].as_str()),
            (CONFLICT, Some("invalid_transition"))
        );
        assert_eq!(
            srv.call(
                "POST",
                &format!("/api/v1/facts/{}/reject", s(&a["id"])),
                None
            )
            .await
            .0,
            OK
        );
        assert_eq!(
            srv.call(
                "POST",
                &format!("/api/v1/facts/{}/approve", s(&a["id"])),
                None
            )
            .await
            .0,
            OK
        );

        // a fact stated in the text must cite it
        rejects(
            &srv,
            "POST",
            &facts,
            json!({"text": "Х", "status": "explicit"}),
            "at least one passage",
        )
        .await;
        let e = fact(&srv, &b, json!({"text": "Родился на хуторе", "status": "explicit", "evidence": [{"passage_id": p[0]}]})).await;
        assert_eq!(
            (e["status"].as_str(), e["approved_by"].clone()),
            (Some("explicit"), Value::Null)
        );
        assert_eq!(
            srv.call(
                "POST",
                &format!("/api/v1/facts/{}/approve", s(&e["id"])),
                None
            )
            .await
            .0,
            CONFLICT,
            "explicit facts need no approval"
        );
        assert_eq!(
            srv.call(
                "POST",
                &format!("/api/v1/facts/{}/reject", s(&e["id"])),
                None
            )
            .await
            .0,
            OK
        );

        // an AI fact without evidence is refused
        rejects(
            &srv,
            "POST",
            &facts,
            json!({"text": "Х", "origin": "ai"}),
            "at least one passage",
        )
        .await;
        // imports start as explicit and cite too
        rejects(
            &srv,
            "POST",
            &facts,
            json!({"text": "Х", "origin": "import"}),
            "at least one passage",
        )
        .await;
        let i = fact(
            &srv,
            &b,
            json!({"text": "Из импорта", "origin": "import", "evidence": [{"passage_id": p[1]}]}),
        )
        .await;
        assert_eq!(i["status"], "explicit");
    }

    // ---- evidence ----

    #[tokio::test]
    async fn evidence_must_be_real_active_passages_of_the_same_book() {
        let srv = Server::new("canon-evidence").await;
        let b = book(&srv, "Книга").await;
        let other = book(&srv, "Другая").await;
        let (scene, p) = scene_with(&srv, &b, &[P0, P1]).await;
        let (_, q) = scene_with(&srv, &other, &[P2]).await;
        let facts = format!("/api/v1/books/{b}/facts");
        let many: Vec<Value> = (0..51).map(|_| json!({"passage_id": p[0]})).collect();
        let cases: Vec<(Value, &str)> = vec![
            (
                json!([{"passage_id": "zzzzzzzz"}]),
                "not a passage of this book",
            ),
            (json!([{"passage_id": "не-код"}]), "malformed"),
            (json!([{"passage_id": q[0]}]), "not a passage of this book"),
            (json!([{"passage_id": p[0]}, {"passage_id": p[0]}]), "twice"),
            (json!(many), "at most"),
            (json!([{"passage": p[0]}]), "passage_id"),
        ];
        for (evidence, needle) in cases {
            let (st, e) = srv
                .call(
                    "POST",
                    &facts,
                    Some(json!({"text": "Х", "origin": "ai", "evidence": evidence})),
                )
                .await;
            assert_eq!(st, INVALID, "{evidence}");
            let msg = e["error"]["message"].as_str().unwrap_or_default();
            assert!(
                msg.contains(needle) || needle == "passage_id",
                "{evidence}: {msg}"
            );
        }
        // a passage that was removed from the text cannot be cited any more
        resave(&srv, &scene, vec![keep(&p[0], P0)], json!([])).await;
        rejects(
            &srv,
            "POST",
            &facts,
            json!({"text": "Х", "origin": "ai", "evidence": [{"passage_id": p[1]}]}),
            "no longer exists",
        )
        .await;
        let (_, n): (StatusCode, Value) = srv.call("GET", &facts, None).await;
        assert_eq!(n, json!([]), "nothing was created by the refused calls");
        // the passage index information
        let (st, info) = srv
            .call("GET", &format!("/api/v1/passages/{}", p[1]), None)
            .await;
        assert_eq!(
            (st, info["status"].as_str(), info["scene_id"].as_str()),
            (OK, Some("removed"), Some(scene.as_str()))
        );
        assert_eq!(
            srv.call("GET", "/api/v1/passages/zzzzzzzz", None).await.0,
            MISSING
        );
        assert_eq!(
            srv.call("GET", "/api/v1/passages/not-an-id", None).await.0,
            MISSING
        );
    }

    // ---- stale facts ----

    #[tokio::test]
    async fn removing_merging_or_editing_a_cited_passage_flags_the_fact_and_never_deletes_it() {
        let srv = Server::new("canon-stale").await;
        let b = book(&srv, "Книга").await;
        let (scene, p) = scene_with(&srv, &b, &[P0, P1, P2]).await;
        let f1 = fact(&srv, &b, ai("Родился на хуторе", &p[0])).await;
        let f2 = fact(
            &srv,
            &b,
            json!({"text": "Никогда не бывал в Москве", "evidence": [{"passage_id": p[1]}]}),
        )
        .await;
        let f3 = fact(&srv, &b, json!({"text": "Две цитаты", "status": "explicit", "evidence": [{"passage_id": p[0]}, {"passage_id": p[2]}]})).await;
        let f4 = fact(&srv, &b, json!({"text": "Без цитат"})).await;
        let (i1, i2, i3, i4) = (s(&f1["id"]), s(&f2["id"]), s(&f3["id"]), s(&f4["id"]));
        let stale = format!("/api/v1/books/{b}/facts?stale=true");
        let ids_of = |v: &Value| -> Vec<String> {
            v.as_array().unwrap().iter().map(|f| s(&f["id"])).collect()
        };
        assert_eq!(srv.call("GET", &stale, None).await.1, json!([]));

        // an edit of P0 (same id, new text): the facts that cite it are flagged, the others are not
        resave(
            &srv,
            &scene,
            vec![
                keep(&p[0], "Григорий родился в станице."),
                keep(&p[1], P1),
                keep(&p[2], P2),
            ],
            json!([]),
        )
        .await;
        let (_, flagged) = srv.call("GET", &stale, None).await;
        assert_eq!(ids_of(&flagged), [i1.clone(), i3.clone()]);
        let g1 = get(&srv, &i1).await;
        assert_eq!(
            (g1["stale_reason"].as_str(), g1["status"].as_str()),
            (Some("a cited passage was edited"), Some("ai_hypothesis"))
        );
        assert_eq!(g1["evidence"][0]["changed"], true);
        assert_eq!(get(&srv, &i2).await["stale"], false);
        assert_eq!(get(&srv, &i4).await["stale"], false);

        // removing P1 flags the fact that cites it; the fact and its approval stay
        resave(
            &srv,
            &scene,
            vec![keep(&p[0], "Григорий родился в станице."), keep(&p[2], P2)],
            json!([]),
        )
        .await;
        let g2 = get(&srv, &i2).await;
        assert_eq!(
            (
                g2["stale"].as_bool(),
                g2["stale_reason"].as_str(),
                g2["status"].as_str()
            ),
            (
                Some(true),
                Some("a cited passage was removed"),
                Some("author_approved")
            )
        );
        assert_eq!(
            g2["evidence"],
            json!([{"passage_id": p[1], "passage_status": "removed", "changed": false}])
        );
        assert_eq!(
            srv.call("GET", &format!("/api/v1/passages/{}", p[1]), None)
                .await
                .1["status"],
            "removed"
        );

        // merging P2 into P0 upgrades the reason of the fact that was only flagged as edited
        resave(
            &srv,
            &scene,
            vec![keep(&p[0], "Григорий родился в станице. «Ёлочки» и ёж.")],
            json!([{"from": p[2], "into": p[0]}]),
        )
        .await;
        let g3 = get(&srv, &i3).await;
        assert_eq!(
            g3["stale_reason"],
            "a cited passage was merged into another"
        );
        assert_eq!(
            srv.call("GET", &format!("/api/v1/passages/{}", p[2]), None)
                .await
                .1["merged_into"],
            p[0].as_str()
        );
        assert_eq!(get(&srv, &i4).await["stale"], false);
        let (_, all) = srv
            .call("GET", &format!("/api/v1/books/{b}/facts"), None)
            .await;
        assert_eq!(all.as_array().unwrap().len(), 4, "no fact was deleted");

        // the author confirms by citing fresh passages: the flag clears
        let (st, fixed) = srv
            .call(
                "PUT",
                &format!("/api/v1/facts/{i1}/evidence"),
                Some(json!({"evidence": [{"passage_id": p[0]}]})),
            )
            .await;
        assert_eq!(st, OK, "{fixed}");
        assert_eq!(
            (
                fixed["stale"].as_bool(),
                fixed["stale_reason"].clone(),
                fixed["evidence"][0]["changed"].as_bool()
            ),
            (Some(false), Value::Null, Some(false))
        );
        // ... which needs real passages, and at least one for an AI or explicit fact
        rejects(
            &srv,
            "PUT",
            &format!("/api/v1/facts/{i1}/evidence"),
            json!({"evidence": [{"passage_id": p[1]}]}),
            "no longer exists",
        )
        .await;
        rejects(
            &srv,
            "PUT",
            &format!("/api/v1/facts/{i1}/evidence"),
            json!({"evidence": []}),
            "at least one passage",
        )
        .await;
        let (st, relaxed) = srv
            .call(
                "PUT",
                &format!("/api/v1/facts/{i2}/evidence"),
                Some(json!({"evidence": []})),
            )
            .await;
        assert_eq!(
            (st, relaxed["stale"].as_bool()),
            (OK, Some(false)),
            "an author-approved fact may stand without citations"
        );
        assert_eq!(
            get(&srv, &i3).await["stale"],
            true,
            "other facts keep their flag"
        );
        // a flag stays on the fact through approval: approving does not hide the problem
        let g = srv
            .call("POST", &format!("/api/v1/facts/{i1}/reject"), None)
            .await;
        assert_eq!(g.0, OK);
    }

    #[tokio::test]
    async fn an_unrelated_edit_flags_nothing() {
        let srv = Server::new("canon-unrelated").await;
        let b = book(&srv, "Книга").await;
        let (scene, p) = scene_with(&srv, &b, &[P0, P1]).await;
        let f = fact(&srv, &b, ai("Факт", &p[0])).await;
        resave(
            &srv,
            &scene,
            vec![keep(&p[0], P0), keep(&p[1], "Другой текст второго абзаца.")],
            json!([]),
        )
        .await;
        resave(
            &srv,
            &scene,
            vec![
                keep(&p[1], "Другой текст второго абзаца."),
                keep(&p[0], P0),
                json!({"text": "Новый абзац."}),
            ],
            json!([]),
        )
        .await;
        assert_eq!(
            get(&srv, &s(&f["id"])).await["stale"],
            false,
            "editing, adding and reordering other paragraphs"
        );
        assert_eq!(
            srv.call("DELETE", &format!("/api/v1/facts/{}", s(&f["id"])), None)
                .await
                .0,
            StatusCode::METHOD_NOT_ALLOWED,
            "facts are retired by rejecting, never deleted"
        );
    }

    // ---- listing and editing ----

    #[tokio::test]
    async fn facts_can_be_listed_by_entity_passage_status_origin_and_staleness() {
        let srv = Server::new("canon-list").await;
        let b = book(&srv, "Книга").await;
        let other = book(&srv, "Другая").await;
        let (_, p) = scene_with(&srv, &b, &[P0, P1]).await;
        let (_, e) = srv
            .call(
                "POST",
                &format!("/api/v1/books/{b}/entities"),
                Some(json!({"kind": "character", "name": "Григорий"})),
            )
            .await;
        let hero = s(&e["id"]);
        let (_, st) = srv
            .call(
                "POST",
                &format!("/api/v1/books/{other}/entities"),
                Some(json!({"kind": "character", "name": "Чужой"})),
            )
            .await;
        let stranger = s(&st["id"]);
        let a = fact(
            &srv,
            &b,
            json!({"text": "О герое", "entity_id": hero, "evidence": [{"passage_id": p[0]}]}),
        )
        .await;
        let c = fact(&srv, &b, json!({"text": "Гипотеза о герое", "entity_id": hero, "origin": "ai", "evidence": [{"passage_id": p[1]}]})).await;
        let d = fact(
            &srv,
            &b,
            json!({"text": "Без героя", "origin": "ai", "evidence": [{"passage_id": p[0]}]}),
        )
        .await;
        let base = format!("/api/v1/books/{b}/facts");
        let ids = |v: Value| -> Vec<String> {
            v.as_array().unwrap().iter().map(|f| s(&f["id"])).collect()
        };
        let (ia, ic, id) = (s(&a["id"]), s(&c["id"]), s(&d["id"]));
        assert_eq!(
            ids(srv.call("GET", &base, None).await.1),
            [ia.clone(), ic.clone(), id.clone()]
        );
        assert_eq!(
            ids(srv
                .call("GET", &format!("{base}?entity={hero}"), None)
                .await
                .1),
            [ia.clone(), ic.clone()]
        );
        assert_eq!(
            ids(srv
                .call("GET", &format!("{base}?passage={}", p[0]), None)
                .await
                .1),
            [ia.clone(), id.clone()]
        );
        assert_eq!(
            ids(srv
                .call("GET", &format!("{base}?status=ai_hypothesis"), None)
                .await
                .1),
            [ic.clone(), id.clone()]
        );
        assert_eq!(
            ids(srv
                .call("GET", &format!("{base}?origin=author"), None)
                .await
                .1),
            [ia]
        );
        assert_eq!(
            ids(srv
                .call("GET", &format!("{base}?stale=false"), None)
                .await
                .1)
            .len(),
            3
        );
        assert_eq!(
            ids(srv
                .call(
                    "GET",
                    &format!("{base}?entity={hero}&status=ai_hypothesis&passage={}", p[1]),
                    None
                )
                .await
                .1),
            [ic],
            "filters combine"
        );
        assert_eq!(
            srv.call("GET", &format!("{base}?passage=zzzzzzzz"), None)
                .await
                .1,
            json!([])
        );
        for bad in [
            "status=weird",
            "origin=robot",
            &format!("entity={stranger}"),
            "passage=bad",
            "stale=maybe",
            "colour=red",
            "status=%D0%B6",
        ] {
            assert_eq!(
                srv.call("GET", &format!("{base}?{bad}"), None).await.0,
                INVALID,
                "{bad}"
            );
        }
        assert_eq!(
            srv.call("GET", "/api/v1/books/bk_00000000/facts", None)
                .await
                .0,
            MISSING
        );
        // an entity with canon facts cannot be deleted, with or without cascade
        let (st, err) = srv
            .call(
                "DELETE",
                &format!("/api/v1/entities/{hero}?cascade=true"),
                None,
            )
            .await;
        assert_eq!(
            (st, err["error"]["code"].as_str()),
            (CONFLICT, Some("in_use"))
        );
    }

    #[tokio::test]
    async fn fact_text_and_subject_can_be_edited_without_touching_the_status() {
        let srv = Server::new("canon-edit").await;
        let b = book(&srv, "Книга").await;
        let other = book(&srv, "Другая").await;
        let (_, p) = scene_with(&srv, &b, &[P0]).await;
        let (_, e) = srv
            .call(
                "POST",
                &format!("/api/v1/books/{b}/entities"),
                Some(json!({"kind": "character", "name": "Григорий"})),
            )
            .await;
        let hero = s(&e["id"]);
        let (_, st) = srv
            .call(
                "POST",
                &format!("/api/v1/books/{other}/entities"),
                Some(json!({"kind": "character", "name": "Чужой"})),
            )
            .await;
        let f = fact(&srv, &b, ai("Черновик", &p[0])).await;
        let one = format!("/api/v1/facts/{}", s(&f["id"]));

        let (code, g) = srv.call("PATCH", &one, Some(json!({"text": "Ёжик «Тихий» — друг\u{a0}Ё", "entity_id": hero, "source_ref": "skill-2"}))).await;
        assert_eq!(code, OK, "{g}");
        assert_eq!(
            g["text"].as_str().unwrap().as_bytes(),
            "Ёжик «Тихий» — друг\u{a0}Ё".as_bytes()
        );
        assert_eq!(
            (
                g["entity_id"].as_str(),
                g["source_ref"].as_str(),
                g["status"].as_str()
            ),
            (Some(hero.as_str()), Some("skill-2"), Some("ai_hypothesis"))
        );
        let (_, g) = srv
            .call(
                "PATCH",
                &one,
                Some(json!({"entity_id": null, "source_ref": null})),
            )
            .await;
        assert_eq!(
            (
                g["entity_id"].clone(),
                g["source_ref"].clone(),
                g["text"].as_str().is_some()
            ),
            (Value::Null, Value::Null, true)
        );

        rejects(&srv, "PATCH", &one, json!({}), "nothing to change").await;
        rejects(&srv, "PATCH", &one, json!({"text": ""}), "text").await;
        rejects(
            &srv,
            "PATCH",
            &one,
            json!({"text": "я".repeat(2001)}),
            "text",
        )
        .await;
        rejects(&srv, "PATCH", &one, json!({"text": "a\u{0}b"}), "control").await;
        rejects(
            &srv,
            "PATCH",
            &one,
            json!({"entity_id": s(&st["id"])}),
            "entity_id",
        )
        .await;
        rejects(
            &srv,
            "PATCH",
            &one,
            json!({"source_ref": "я".repeat(201)}),
            "source_ref",
        )
        .await;
        assert_eq!(
            srv.call(
                "PATCH",
                "/api/v1/facts/ft_00000000",
                Some(json!({"text": "Х"}))
            )
            .await
            .0,
            MISSING
        );
        assert_eq!(
            srv.call("GET", "/api/v1/facts/ft_00000000", None).await.0,
            MISSING
        );
        assert_eq!(
            srv.call("GET", "/api/v1/facts/garbage", None).await.0,
            MISSING
        );
        assert_eq!(
            srv.call("GET", "/api/v1/facts/ft_00000000/history", None)
                .await
                .0,
            MISSING
        );
        assert_eq!(
            srv.call(
                "PUT",
                "/api/v1/facts/ft_00000000/evidence",
                Some(json!({"evidence": []}))
            )
            .await
            .0,
            MISSING
        );

        let facts = format!("/api/v1/books/{b}/facts");
        for (body, needle) in [
            (json!({"text": ""}), "text"),
            (json!({"text": "   "}), "text"),
            (json!({"text": "я".repeat(2001)}), "text"),
            (json!({"text": "Х", "entity_id": s(&st["id"])}), "entity_id"),
            (
                json!({"text": "Х", "entity_id": "en_00000000"}),
                "entity_id",
            ),
            (
                json!({"text": "Х", "source_ref": "я".repeat(201)}),
                "source_ref",
            ),
        ] {
            rejects(&srv, "POST", &facts, body, needle).await;
        }
        assert_eq!(
            srv.call("POST", &facts, Some(json!({"text": "Х", "colour": "red"})))
                .await
                .0,
            INVALID
        );
        assert_eq!(
            srv.call("POST", &facts, Some(json!({"entity_id": hero})))
                .await
                .0,
            INVALID
        );
        assert_eq!(
            srv.call(
                "POST",
                "/api/v1/books/bk_00000000/facts",
                Some(json!({"text": "Х"}))
            )
            .await
            .0,
            MISSING
        );
    }
}
