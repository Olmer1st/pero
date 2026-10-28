//! The canon ledger: facts about the story, each with a status, evidence passages and a status
//! history. SQLite only; the prose stays in the book repositories.
//!
//! Statuses: `explicit` (stated in the text), `author_approved`, `ai_hypothesis`, `rejected`.
//! The one rule that matters: an AI hypothesis becomes canon only through `approve`, an explicit
//! act that records who approved and when. Nothing else can set `author_approved` on an AI fact.

use std::collections::HashMap;

use serde::{Deserialize, Deserializer, Serialize};
use sqlx::{Row, SqliteConnection, SqlitePool};

use crate::{ids, manuscript, store::StoreError};

const MAX_TEXT_CHARS: usize = 2000;
const MAX_EVIDENCE: usize = 50;
const MAX_REASON_CHARS: usize = 500;
const MAX_SOURCE_CHARS: usize = 200;

fn invalid(m: impl Into<String>) -> StoreError {
    StoreError::Invalid(m.into())
}

fn double_option<'de, T, D>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Deserialize::deserialize(d).map(Some)
}

// ---- data shown to clients -------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EvidenceOut {
    pub passage_id: String,
    /// `active`, or `removed` when the cited passage no longer exists in the text.
    pub passage_status: String,
    /// True when the passage text changed since it was cited.
    pub changed: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Fact {
    pub id: String,
    pub book_id: String,
    pub entity_id: Option<String>,
    pub text: String,
    pub status: String,
    pub origin: String,
    pub source_ref: Option<String>,
    /// True when a cited passage was removed, merged or edited since the fact was recorded.
    pub stale: bool,
    pub stale_reason: Option<String>,
    pub approved_by: Option<String>,
    pub approved_at: Option<String>,
    pub evidence: Vec<EvidenceOut>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct HistoryEntry {
    pub from_status: Option<String>,
    pub to_status: String,
    pub changed_by: String,
    pub reason: Option<String>,
    pub changed_at: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PassageInfo {
    pub id: String,
    pub book_id: String,
    pub scene_id: String,
    pub position: i64,
    pub status: String,
    pub merged_into: Option<String>,
    pub removed_at: Option<String>,
}

#[derive(sqlx::FromRow)]
struct FactRow {
    id: String,
    book_id: String,
    entity_id: Option<String>,
    text: String,
    status: String,
    origin: String,
    source_ref: Option<String>,
    stale: i64,
    stale_reason: Option<String>,
    approved_by: Option<String>,
    approved_at: Option<String>,
    created_at: String,
    updated_at: String,
}

impl FactRow {
    fn with(self, evidence: Vec<EvidenceOut>) -> Fact {
        Fact {
            id: self.id,
            book_id: self.book_id,
            entity_id: self.entity_id,
            text: self.text,
            status: self.status,
            origin: self.origin,
            source_ref: self.source_ref,
            stale: self.stale != 0,
            stale_reason: self.stale_reason,
            approved_by: self.approved_by,
            approved_at: self.approved_at,
            evidence,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

macro_rules! fact_columns {
    () => {
        "id, book_id, entity_id, text, status, origin, source_ref, stale, stale_reason, approved_by, \
         approved_at, created_at, updated_at"
    };
}

// ---- requests --------------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceIn {
    pub passage_id: String,
}

/// `status` is optional: the author's facts default to `author_approved`, AI facts are always
/// `ai_hypothesis`. Approval fields cannot be sent; only `approve` writes them.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewFact {
    pub text: String,
    pub entity_id: Option<String>,
    pub status: Option<String>,
    /// `author` (default), `ai` or `import`.
    pub origin: Option<String>,
    /// For AI facts: the skill or model that proposed it.
    pub source_ref: Option<String>,
    #[serde(default)]
    pub evidence: Vec<EvidenceIn>,
}

/// Text and subject can be edited; the status can only change through `approve`, `reject`, `reopen`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactPatch {
    pub text: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    pub entity_id: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub source_ref: Option<Option<String>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransitionBody {
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceReplace {
    pub evidence: Vec<EvidenceIn>,
}

/// Filters for listing; every filter is optional and they combine with AND.
#[derive(Debug, Default)]
pub struct FactFilter {
    pub status: Option<String>,
    pub entity: Option<String>,
    pub passage: Option<String>,
    pub origin: Option<String>,
    pub stale: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Approve,
    Reject,
    Reopen,
}

// ---- validation helpers ----------------------------------------------------------------------

fn text_of(value: &str) -> Result<String, StoreError> {
    let v = value.trim();
    if v.is_empty() {
        return Err(invalid("text cannot be empty"));
    }
    if v.chars().count() > MAX_TEXT_CHARS {
        return Err(invalid(format!(
            "text is longer than {MAX_TEXT_CHARS} characters"
        )));
    }
    if v.chars()
        .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
    {
        return Err(invalid("text contains control characters"));
    }
    Ok(v.to_string())
}

fn short(label: &str, value: &str, max: usize) -> Result<String, StoreError> {
    let v = manuscript::clean_field(label, value).map_err(invalid)?;
    if v.chars().count() > max {
        return Err(invalid(format!("{label} is longer than {max} characters")));
    }
    Ok(v)
}

fn optional_short(
    label: &str,
    value: &Option<String>,
    max: usize,
) -> Result<Option<String>, StoreError> {
    match value {
        None => Ok(None),
        Some(v) => {
            let c = short(label, v, max)?;
            Ok(if c.is_empty() { None } else { Some(c) })
        }
    }
}

fn check_id(id: &str, prefix: &str, what: &str) -> Result<(), StoreError> {
    if ids::is_id(id, prefix) {
        Ok(())
    } else {
        Err(StoreError::NotFound(format!("{what} not found")))
    }
}

async fn require_book(pool: &SqlitePool, book_id: &str) -> Result<(), StoreError> {
    check_id(book_id, ids::BOOK, "book")?;
    let row: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM books WHERE id = ?")
        .bind(book_id)
        .fetch_optional(pool)
        .await?;
    row.map(|_| ())
        .ok_or_else(|| StoreError::NotFound("book not found".into()))
}

async fn check_entity(pool: &SqlitePool, entity_id: &str, book_id: &str) -> Result<(), StoreError> {
    let row: Option<(String,)> = sqlx::query_as("SELECT book_id FROM entities WHERE id = ?")
        .bind(entity_id)
        .fetch_optional(pool)
        .await?;
    match row {
        Some((b,)) if b == book_id => Ok(()),
        _ => Err(invalid("entity_id: not an entity of this book")),
    }
}

/// Looks up the cited passages: they must exist in the book and still be active. Returns
/// `(passage_id, current text hash)` in the order given.
async fn cited_passages(
    pool: &SqlitePool,
    book_id: &str,
    evidence: &[EvidenceIn],
) -> Result<Vec<(String, String)>, StoreError> {
    if evidence.len() > MAX_EVIDENCE {
        return Err(invalid(format!("at most {MAX_EVIDENCE} evidence passages")));
    }
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(evidence.len());
    for e in evidence {
        if !ids::is_code(&e.passage_id) {
            return Err(invalid("evidence: malformed passage id"));
        }
        if !seen.insert(e.passage_id.as_str()) {
            return Err(invalid("evidence: the same passage is cited twice"));
        }
        let row: Option<(String, String, String)> =
            sqlx::query_as("SELECT book_id, status, text_hash FROM passages WHERE id = ?")
                .bind(&e.passage_id)
                .fetch_optional(pool)
                .await?;
        match row {
            Some((b, status, hash)) if b == book_id => {
                if status != "active" {
                    return Err(invalid(
                        "evidence: a cited passage no longer exists in the text",
                    ));
                }
                out.push((e.passage_id.clone(), hash));
            }
            _ => return Err(invalid("evidence: not a passage of this book")),
        }
    }
    Ok(out)
}

fn needs_evidence(status: &str) -> bool {
    matches!(status, "explicit" | "ai_hypothesis")
}

// ---- reading ---------------------------------------------------------------------------------

async fn evidence_of_book(
    pool: &SqlitePool,
    book_id: &str,
) -> Result<HashMap<String, Vec<EvidenceOut>>, StoreError> {
    let rows = sqlx::query(
        "SELECT e.fact_id, e.passage_id, e.text_hash AS cited, p.status AS passage_status, p.text_hash AS current \
         FROM fact_evidence e JOIN passages p ON p.id = e.passage_id JOIN facts f ON f.id = e.fact_id \
         WHERE f.book_id = ? ORDER BY e.fact_id, e.passage_id",
    )
    .bind(book_id)
    .fetch_all(pool)
    .await?;
    let mut out: HashMap<String, Vec<EvidenceOut>> = HashMap::new();
    for r in rows {
        let cited: String = r.get("cited");
        let current: String = r.get("current");
        out.entry(r.get("fact_id")).or_default().push(EvidenceOut {
            passage_id: r.get("passage_id"),
            passage_status: r.get("passage_status"),
            changed: cited != current,
        });
    }
    Ok(out)
}

async fn evidence_of_fact(
    pool: &SqlitePool,
    fact_id: &str,
) -> Result<Vec<EvidenceOut>, StoreError> {
    let rows = sqlx::query(
        "SELECT e.passage_id, e.text_hash AS cited, p.status AS passage_status, p.text_hash AS current \
         FROM fact_evidence e JOIN passages p ON p.id = e.passage_id WHERE e.fact_id = ? ORDER BY e.passage_id",
    )
    .bind(fact_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .map(|r| EvidenceOut {
            passage_id: r.get("passage_id"),
            passage_status: r.get("passage_status"),
            changed: r.get::<String, _>("cited") != r.get::<String, _>("current"),
        })
        .collect())
}

pub async fn get_fact(pool: &SqlitePool, id: &str) -> Result<Fact, StoreError> {
    check_id(id, ids::FACT, "fact")?;
    let row: FactRow = sqlx::query_as(concat!(
        "SELECT ",
        fact_columns!(),
        " FROM facts WHERE id = ?"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| StoreError::NotFound("fact not found".into()))?;
    let evidence = evidence_of_fact(pool, id).await?;
    Ok(row.with(evidence))
}

pub async fn list_facts(
    pool: &SqlitePool,
    book_id: &str,
    filter: &FactFilter,
) -> Result<Vec<Fact>, StoreError> {
    require_book(pool, book_id).await?;
    if let Some(s) = &filter.status
        && !STATUSES.contains(&s.as_str())
    {
        return Err(invalid(
            "status must be explicit, author_approved, ai_hypothesis or rejected",
        ));
    }
    if let Some(o) = &filter.origin
        && !ORIGINS.contains(&o.as_str())
    {
        return Err(invalid("origin must be author, ai or import"));
    }
    if let Some(e) = &filter.entity {
        check_entity(pool, e, book_id).await?;
    }
    if let Some(p) = &filter.passage
        && !ids::is_code(p)
    {
        return Err(invalid("passage must be a passage id"));
    }
    let rows: Vec<FactRow> = sqlx::query_as(concat!(
        "SELECT ",
        fact_columns!(),
        " FROM facts WHERE book_id = ?1 AND (?2 IS NULL OR status = ?2) AND (?3 IS NULL OR entity_id = ?3) \
         AND (?4 IS NULL OR id IN (SELECT fact_id FROM fact_evidence WHERE passage_id = ?4)) \
         AND (?5 IS NULL OR origin = ?5) AND (?6 IS NULL OR stale = ?6) ORDER BY created_at, id"
    ))
    .bind(book_id)
    .bind(&filter.status)
    .bind(&filter.entity)
    .bind(&filter.passage)
    .bind(&filter.origin)
    .bind(filter.stale.map(i64::from))
    .fetch_all(pool)
    .await?;
    let mut evidence = evidence_of_book(pool, book_id).await?;
    Ok(rows
        .into_iter()
        .map(|r| {
            let e = evidence.remove(&r.id).unwrap_or_default();
            r.with(e)
        })
        .collect())
}

pub async fn history(pool: &SqlitePool, fact_id: &str) -> Result<Vec<HistoryEntry>, StoreError> {
    get_fact(pool, fact_id).await?;
    Ok(sqlx::query(
        "SELECT from_status, to_status, changed_by, reason, changed_at FROM fact_status_history \
         WHERE fact_id = ? ORDER BY id",
    )
    .bind(fact_id)
    .fetch_all(pool)
    .await?
    .iter()
    .map(|r| HistoryEntry {
        from_status: r.get("from_status"),
        to_status: r.get("to_status"),
        changed_by: r.get("changed_by"),
        reason: r.get("reason"),
        changed_at: r.get("changed_at"),
    })
    .collect())
}

pub async fn get_passage(pool: &SqlitePool, id: &str) -> Result<PassageInfo, StoreError> {
    if !ids::is_code(id) {
        return Err(StoreError::NotFound("passage not found".into()));
    }
    let row = sqlx::query(
        "SELECT id, book_id, scene_id, position, status, merged_into, removed_at FROM passages WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| StoreError::NotFound("passage not found".into()))?;
    Ok(PassageInfo {
        id: row.get("id"),
        book_id: row.get("book_id"),
        scene_id: row.get("scene_id"),
        position: row.get("position"),
        status: row.get("status"),
        merged_into: row.get("merged_into"),
        removed_at: row.get("removed_at"),
    })
}

// ---- writing ---------------------------------------------------------------------------------

pub const STATUSES: [&str; 4] = ["explicit", "author_approved", "ai_hypothesis", "rejected"];
const ORIGINS: [&str; 3] = ["author", "ai", "import"];

/// Decides the starting status. The author's own facts default to `author_approved` (entering a
/// fact is the explicit act); an AI fact can only start as `ai_hypothesis`.
fn starting_status(origin: &str, requested: Option<&str>) -> Result<&'static str, StoreError> {
    match (origin, requested) {
        ("author", None | Some("author_approved")) => Ok("author_approved"),
        ("author", Some("explicit")) => Ok("explicit"),
        ("author", Some(_)) => Err(invalid(
            "a fact entered by the author starts as author_approved or explicit",
        )),
        ("ai", None | Some("ai_hypothesis")) => Ok("ai_hypothesis"),
        ("ai", Some(_)) => Err(invalid(
            "a fact proposed by AI always starts as ai_hypothesis; only the author can approve it",
        )),
        ("import", None | Some("explicit")) => Ok("explicit"),
        ("import", Some(_)) => Err(invalid("an imported fact starts as explicit")),
        _ => Err(invalid("origin must be author, ai or import")),
    }
}

pub async fn create_fact(
    pool: &SqlitePool,
    book_id: &str,
    new: &NewFact,
    actor: &str,
) -> Result<Fact, StoreError> {
    require_book(pool, book_id).await?;
    let text = text_of(&new.text)?;
    let origin = new.origin.as_deref().unwrap_or("author");
    let status = starting_status(origin, new.status.as_deref())?;
    let source_ref = optional_short("source_ref", &new.source_ref, MAX_SOURCE_CHARS)?;
    if let Some(e) = &new.entity_id {
        check_entity(pool, e, book_id).await?;
    }
    let cited = cited_passages(pool, book_id, &new.evidence).await?;
    if needs_evidence(status) && cited.is_empty() {
        return Err(invalid(
            "a fact that is stated in the text or proposed by AI must cite at least one passage",
        ));
    }
    let changed_by = match origin {
        "author" => actor.to_string(),
        "ai" => format!("ai:{}", source_ref.as_deref().unwrap_or("unknown")),
        _ => "import".to_string(),
    };
    let approved_by = (status == "author_approved").then_some(actor);
    let id = ids::new_id(ids::FACT);
    let mut tx = pool.begin().await?;
    sqlx::query(
        "INSERT INTO facts (id, book_id, entity_id, text, status, origin, source_ref, approved_by, approved_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, CASE WHEN ?8 IS NULL THEN NULL ELSE strftime('%Y-%m-%dT%H:%M:%fZ', 'now') END)",
    )
    .bind(&id)
    .bind(book_id)
    .bind(&new.entity_id)
    .bind(&text)
    .bind(status)
    .bind(origin)
    .bind(&source_ref)
    .bind(approved_by)
    .execute(&mut *tx)
    .await?;
    for (passage_id, hash) in &cited {
        sqlx::query("INSERT INTO fact_evidence (fact_id, passage_id, text_hash) VALUES (?, ?, ?)")
            .bind(&id)
            .bind(passage_id)
            .bind(hash)
            .execute(&mut *tx)
            .await?;
    }
    record(&mut tx, &id, None, status, &changed_by, None).await?;
    tx.commit().await?;
    get_fact(pool, &id).await
}

async fn record(
    conn: &mut SqliteConnection,
    fact_id: &str,
    from: Option<&str>,
    to: &str,
    by: &str,
    reason: Option<&str>,
) -> Result<(), StoreError> {
    sqlx::query(
        "INSERT INTO fact_status_history (fact_id, from_status, to_status, changed_by, reason) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(fact_id)
    .bind(from)
    .bind(to)
    .bind(by)
    .bind(reason)
    .execute(conn)
    .await?;
    Ok(())
}

pub async fn update_fact(
    pool: &SqlitePool,
    id: &str,
    patch: &FactPatch,
) -> Result<Fact, StoreError> {
    let current = get_fact(pool, id).await?;
    if patch.text.is_none() && patch.entity_id.is_none() && patch.source_ref.is_none() {
        return Err(invalid("nothing to change"));
    }
    let text = match &patch.text {
        Some(t) => text_of(t)?,
        None => current.text.clone(),
    };
    let entity_id = match &patch.entity_id {
        Some(Some(e)) => {
            check_entity(pool, e, &current.book_id).await?;
            Some(e.clone())
        }
        Some(None) => None,
        None => current.entity_id.clone(),
    };
    let source_ref = match &patch.source_ref {
        Some(s) => optional_short("source_ref", s, MAX_SOURCE_CHARS)?,
        None => current.source_ref.clone(),
    };
    sqlx::query(
        "UPDATE facts SET text = ?, entity_id = ?, source_ref = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE id = ?",
    )
    .bind(&text)
    .bind(&entity_id)
    .bind(&source_ref)
    .bind(id)
    .execute(pool)
    .await?;
    get_fact(pool, id).await
}

/// Approve, reject or reopen a fact. `approve` is the only way a fact that did not start as the
/// author's own becomes `author_approved`; it records who and when.
///   approve: ai_hypothesis or rejected  to author_approved
///   reject:  anything but rejected      to rejected
///   reopen:  author_approved or rejected to ai_hypothesis, only for facts that came from AI
pub async fn transition(
    pool: &SqlitePool,
    id: &str,
    action: Action,
    actor: &str,
    reason: Option<&str>,
) -> Result<Fact, StoreError> {
    let current = get_fact(pool, id).await?;
    let reason = optional_short("reason", &reason.map(String::from), MAX_REASON_CHARS)?;
    let from = current.status.as_str();
    let (to, allowed) = match action {
        Action::Approve => (
            "author_approved",
            matches!(from, "ai_hypothesis" | "rejected"),
        ),
        Action::Reject => ("rejected", from != "rejected"),
        Action::Reopen => (
            "ai_hypothesis",
            current.origin == "ai" && matches!(from, "author_approved" | "rejected"),
        ),
    };
    if !allowed {
        return Err(StoreError::Transition(format!(
            "a fact that is {from} cannot be {}",
            match action {
                Action::Approve => "approved",
                Action::Reject => "rejected",
                Action::Reopen => "reopened (only facts proposed by AI can be)",
            }
        )));
    }
    let mut tx = pool.begin().await?;
    if to == "author_approved" {
        sqlx::query(
            "UPDATE facts SET status = ?, approved_by = ?, approved_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), \
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
        )
        .bind(to)
        .bind(actor)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    } else {
        sqlx::query(
            "UPDATE facts SET status = ?, approved_by = NULL, approved_at = NULL, \
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
        )
        .bind(to)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    }
    record(&mut tx, id, Some(from), to, actor, reason.as_deref()).await?;
    tx.commit().await?;
    get_fact(pool, id).await
}

/// Replaces the evidence of a fact with fresh citations (current text hashes) and clears the stale
/// flag: this is how the author confirms a fact after its passages changed.
pub async fn replace_evidence(
    pool: &SqlitePool,
    id: &str,
    new: &EvidenceReplace,
) -> Result<Fact, StoreError> {
    let current = get_fact(pool, id).await?;
    let cited = cited_passages(pool, &current.book_id, &new.evidence).await?;
    if needs_evidence(&current.status) && cited.is_empty() {
        return Err(invalid(
            "a fact that is explicit or an AI hypothesis must cite at least one passage",
        ));
    }
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM fact_evidence WHERE fact_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    for (passage_id, hash) in &cited {
        sqlx::query("INSERT INTO fact_evidence (fact_id, passage_id, text_hash) VALUES (?, ?, ?)")
            .bind(id)
            .bind(passage_id)
            .bind(hash)
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query(
        "UPDATE facts SET stale = 0, stale_reason = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE id = ?",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    get_fact(pool, id).await
}

/// Called by the manuscript index after every change: flags (never deletes) the facts that cite a
/// passage that was removed, merged into another, or edited. A fact already flagged keeps its
/// first reason, except that removal and merging take precedence over an earlier edit.
pub async fn flag_stale(conn: &mut SqliteConnection, book_id: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE facts SET stale = 1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), \
         stale_reason = (SELECT CASE WHEN p.merged_into IS NOT NULL THEN 'a cited passage was merged into another' \
                                     ELSE 'a cited passage was removed' END \
                         FROM fact_evidence e JOIN passages p ON p.id = e.passage_id \
                         WHERE e.fact_id = facts.id AND p.status = 'removed' \
                         ORDER BY p.merged_into IS NULL, p.id LIMIT 1) \
         WHERE book_id = ?1 AND id IN (SELECT e.fact_id FROM fact_evidence e JOIN passages p ON p.id = e.passage_id \
                                       WHERE p.book_id = ?1 AND p.status = 'removed') \
           AND (stale = 0 OR stale_reason = 'a cited passage was edited')",
    )
    .bind(book_id)
    .execute(&mut *conn)
    .await?;
    sqlx::query(
        "UPDATE facts SET stale = 1, stale_reason = 'a cited passage was edited', \
         updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE book_id = ?1 AND stale = 0 AND id IN (SELECT e.fact_id FROM fact_evidence e \
             JOIN passages p ON p.id = e.passage_id WHERE p.book_id = ?1 AND p.status = 'active' AND p.text_hash <> e.text_hash)",
    )
    .bind(book_id)
    .execute(&mut *conn)
    .await?;
    Ok(())
}
