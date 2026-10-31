//! The story graph: entities (characters, places, organizations), their aliases, relationships
//! between them, and timeline events. Authored structure kept in SQLite only; the prose lives in the
//! book repositories. Every function validates its input and reports problems as `StoreError`.
pub use pero_shared::ENTITY_KINDS;
pub use pero_shared::graph::{
    Entity, EntityPatch, Event, EventPatch, NewEntity, NewEvent, NewRelationship, Participant,
    Relationship, RelationshipPatch,
};

use std::collections::{HashMap, HashSet};

use sqlx::{Row, SqlitePool};

use crate::{ids, manuscript, store::StoreError};

pub const MAX_ALIASES: usize = 50;
pub const MAX_PARTICIPANTS: usize = 200;
const MAX_DESCRIPTION_CHARS: usize = 5000;
const MAX_KIND_CHARS: usize = 100;

fn invalid(m: impl Into<String>) -> StoreError {
    StoreError::Invalid(m.into())
}

// ---- validation helpers ----------------------------------------------------------------------

fn required(label: &str, value: &str) -> Result<String, StoreError> {
    let v = manuscript::clean_field(label, value).map_err(invalid)?;
    if v.is_empty() {
        return Err(invalid(format!("{label} cannot be empty")));
    }
    Ok(v)
}

fn short_text(label: &str, value: &str, max: usize) -> Result<String, StoreError> {
    let v = required(label, value)?;
    if v.chars().count() > max {
        return Err(invalid(format!("{label} is longer than {max} characters")));
    }
    Ok(v)
}

/// Free text (may have line breaks).
fn description(value: &str) -> Result<String, StoreError> {
    if value.chars().count() > MAX_DESCRIPTION_CHARS {
        return Err(invalid(format!(
            "description is longer than {MAX_DESCRIPTION_CHARS} characters"
        )));
    }
    if value
        .chars()
        .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
    {
        return Err(invalid("description contains control characters"));
    }
    Ok(value.trim().to_string())
}

fn kind_of(value: &str) -> Result<&'static str, StoreError> {
    ENTITY_KINDS
        .iter()
        .find(|k| **k == value)
        .copied()
        .ok_or_else(|| invalid("kind must be \"character\", \"place\" or \"organization\""))
}

fn clean_aliases(aliases: &[String], name: &str) -> Result<Vec<String>, StoreError> {
    if aliases.len() > MAX_ALIASES {
        return Err(invalid(format!("at most {MAX_ALIASES} aliases")));
    }
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(aliases.len());
    for a in aliases {
        let a = required("alias", a)?;
        if a == name {
            return Err(invalid("an alias cannot repeat the name"));
        }
        if !seen.insert(a.clone()) {
            return Err(invalid("aliases must be different from each other"));
        }
        out.push(a);
    }
    out.sort();
    Ok(out)
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

/// Returns the kind of an entity if it exists in the given book.
async fn entity_kind_in_book(
    pool: &SqlitePool,
    entity_id: &str,
    book_id: &str,
    field: &str,
) -> Result<String, StoreError> {
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT book_id, kind FROM entities WHERE id = ?")
            .bind(entity_id)
            .fetch_optional(pool)
            .await?;
    match row {
        Some((b, kind)) if b == book_id => Ok(kind),
        _ => Err(invalid(format!("{field}: not an entity of this book"))),
    }
}

// ---- entities --------------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct EntityRow {
    id: String,
    book_id: String,
    kind: String,
    name: String,
    description: String,
    created_at: String,
    updated_at: String,
}

impl EntityRow {
    fn with(self, aliases: Vec<String>) -> Entity {
        Entity {
            id: self.id,
            book_id: self.book_id,
            kind: self.kind,
            name: self.name,
            description: self.description,
            aliases,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

macro_rules! entity_columns {
    () => {
        "id, book_id, kind, name, description, created_at, updated_at"
    };
}

pub async fn create_entity(
    pool: &SqlitePool,
    book_id: &str,
    new: &NewEntity,
) -> Result<Entity, StoreError> {
    require_book(pool, book_id).await?;
    let kind = kind_of(&new.kind)?;
    let name = short_text("name", &new.name, manuscript::MAX_FIELD_CHARS)?;
    let desc = description(&new.description)?;
    let aliases = clean_aliases(&new.aliases, &name)?;
    let id = ids::new_id(ids::ENTITY);
    let mut tx = pool.begin().await?;
    sqlx::query(
        "INSERT INTO entities (id, book_id, kind, name, description) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(book_id)
    .bind(kind)
    .bind(&name)
    .bind(&desc)
    .execute(&mut *tx)
    .await?;
    for a in &aliases {
        sqlx::query("INSERT INTO entity_aliases (entity_id, alias) VALUES (?, ?)")
            .bind(&id)
            .bind(a)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    get_entity(pool, &id).await
}

pub async fn get_entity(pool: &SqlitePool, id: &str) -> Result<Entity, StoreError> {
    check_id(id, ids::ENTITY, "entity")?;
    let row: EntityRow = sqlx::query_as(concat!(
        "SELECT ",
        entity_columns!(),
        " FROM entities WHERE id = ?"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| StoreError::NotFound("entity not found".into()))?;
    let aliases =
        sqlx::query("SELECT alias FROM entity_aliases WHERE entity_id = ? ORDER BY alias")
            .bind(id)
            .fetch_all(pool)
            .await?
            .iter()
            .map(|r| r.get::<String, _>("alias"))
            .collect();
    Ok(row.with(aliases))
}

/// All entities of a book, optionally of one kind, ordered by kind and name (case-insensitive,
/// Cyrillic included).
pub async fn list_entities(
    pool: &SqlitePool,
    book_id: &str,
    kind: Option<&str>,
) -> Result<Vec<Entity>, StoreError> {
    require_book(pool, book_id).await?;
    let kind = kind.map(kind_of).transpose()?;
    let rows: Vec<EntityRow> = sqlx::query_as(concat!(
        "SELECT ",
        entity_columns!(),
        " FROM entities WHERE book_id = ?"
    ))
    .bind(book_id)
    .fetch_all(pool)
    .await?;
    let mut aliases: HashMap<String, Vec<String>> = HashMap::new();
    for r in sqlx::query(
        "SELECT a.entity_id, a.alias FROM entity_aliases a JOIN entities e ON e.id = a.entity_id \
         WHERE e.book_id = ? ORDER BY a.alias",
    )
    .bind(book_id)
    .fetch_all(pool)
    .await?
    {
        aliases
            .entry(r.get("entity_id"))
            .or_default()
            .push(r.get("alias"));
    }
    let mut out: Vec<Entity> = rows
        .into_iter()
        .filter(|r| kind.is_none_or(|k| r.kind == k))
        .map(|r| {
            let a = aliases.remove(&r.id).unwrap_or_default();
            r.with(a)
        })
        .collect();
    out.sort_by_cached_key(|e| (e.kind.clone(), e.name.to_lowercase(), e.id.clone()));
    Ok(out)
}

pub async fn update_entity(
    pool: &SqlitePool,
    id: &str,
    patch: &EntityPatch,
) -> Result<Entity, StoreError> {
    let current = get_entity(pool, id).await?;
    if patch.name.is_none() && patch.description.is_none() && patch.aliases.is_none() {
        return Err(invalid("nothing to change"));
    }
    let name = match &patch.name {
        Some(n) => short_text("name", n, manuscript::MAX_FIELD_CHARS)?,
        None => current.name.clone(),
    };
    let desc = match &patch.description {
        Some(d) => description(d)?,
        None => current.description.clone(),
    };
    let aliases = match &patch.aliases {
        Some(a) => clean_aliases(a, &name)?,
        None => clean_aliases(&current.aliases, &name)?,
    };
    let mut tx = pool.begin().await?;
    sqlx::query(
        "UPDATE entities SET name = ?, description = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE id = ?",
    )
    .bind(&name)
    .bind(&desc)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    if patch.aliases.is_some() {
        sqlx::query("DELETE FROM entity_aliases WHERE entity_id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        for a in &aliases {
            sqlx::query("INSERT INTO entity_aliases (entity_id, alias) VALUES (?, ?)")
                .bind(id)
                .bind(a)
                .execute(&mut *tx)
                .await?;
        }
    }
    tx.commit().await?;
    get_entity(pool, id).await
}

async fn count_uses(
    conn: &mut sqlx::SqliteConnection,
    sql: &'static str,
    id: &str,
) -> Result<i64, StoreError> {
    let (n,): (i64,) = sqlx::query_as(sql).bind(id).fetch_one(conn).await?;
    Ok(n)
}

/// Deletes an entity. Without `cascade`, an entity that is still used is refused. With `cascade`
/// its relationships and timeline participations are removed and place references cleared; canon
/// facts about it always block the deletion (the author must deal with them first).
pub async fn delete_entity(pool: &SqlitePool, id: &str, cascade: bool) -> Result<(), StoreError> {
    get_entity(pool, id).await?;
    let mut tx = pool.begin().await?;
    let (relationships,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM relationships WHERE from_entity_id = ?1 OR to_entity_id = ?1",
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    let participations = count_uses(
        &mut tx,
        "SELECT count(*) FROM timeline_event_participants WHERE entity_id = ?",
        id,
    )
    .await?;
    let places = count_uses(
        &mut tx,
        "SELECT count(*) FROM timeline_events WHERE place_id = ?",
        id,
    )
    .await?;
    let facts = count_uses(
        &mut tx,
        "SELECT count(*) FROM facts WHERE entity_id = ?",
        id,
    )
    .await?;
    if facts > 0 {
        return Err(StoreError::InUse(format!(
            "{facts} canon fact(s) are about this entity; resolve them first"
        )));
    }
    if !cascade && relationships + participations + places > 0 {
        return Err(StoreError::InUse(format!(
            "still used by {relationships} relationship(s), {participations} event participation(s) and {places} event place(s); \
             remove them or delete with cascade"
        )));
    }
    sqlx::query("DELETE FROM relationships WHERE from_entity_id = ? OR to_entity_id = ?")
        .bind(id)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM timeline_event_participants WHERE entity_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "UPDATE timeline_events SET place_id = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE place_id = ?",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM entity_aliases WHERE entity_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM entities WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

// ---- relationships ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct RelationshipRow {
    id: String,
    book_id: String,
    from_entity_id: String,
    to_entity_id: String,
    kind: String,
    directed: i64,
    description: String,
    created_at: String,
    updated_at: String,
}

impl From<RelationshipRow> for Relationship {
    fn from(r: RelationshipRow) -> Self {
        Relationship {
            id: r.id,
            book_id: r.book_id,
            from_entity_id: r.from_entity_id,
            to_entity_id: r.to_entity_id,
            kind: r.kind,
            directed: r.directed != 0,
            description: r.description,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

macro_rules! relationship_columns {
    () => {
        "id, book_id, from_entity_id, to_entity_id, kind, directed, description, created_at, updated_at"
    };
}

/// True if the same relationship already exists (for an undirected one, in either direction).
async fn relationship_exists(
    pool: &SqlitePool,
    book_id: &str,
    from: &str,
    to: &str,
    kind: &str,
    directed: bool,
    except: Option<&str>,
) -> Result<bool, StoreError> {
    let rows = sqlx::query(
        "SELECT id, from_entity_id, directed FROM relationships \
         WHERE book_id = ? AND kind = ? AND ((from_entity_id = ? AND to_entity_id = ?) OR (from_entity_id = ? AND to_entity_id = ?))",
    )
    .bind(book_id)
    .bind(kind)
    .bind(from)
    .bind(to)
    .bind(to)
    .bind(from)
    .fetch_all(pool)
    .await?;
    Ok(rows.iter().any(|r| {
        let same_direction = r.get::<String, _>("from_entity_id") == from;
        let other_directed = r.get::<i64, _>("directed") != 0;
        Some(r.get::<String, _>("id").as_str()) != except
            && (same_direction || !directed || !other_directed)
    }))
}

pub async fn create_relationship(
    pool: &SqlitePool,
    book_id: &str,
    new: &NewRelationship,
) -> Result<Relationship, StoreError> {
    require_book(pool, book_id).await?;
    entity_kind_in_book(pool, &new.from_entity_id, book_id, "from_entity_id").await?;
    entity_kind_in_book(pool, &new.to_entity_id, book_id, "to_entity_id").await?;
    if new.from_entity_id == new.to_entity_id {
        return Err(invalid("a relationship needs two different entities"));
    }
    let kind = short_text("kind", &new.kind, MAX_KIND_CHARS)?;
    let desc = description(&new.description)?;
    if relationship_exists(
        pool,
        book_id,
        &new.from_entity_id,
        &new.to_entity_id,
        &kind,
        new.directed,
        None,
    )
    .await?
    {
        return Err(StoreError::Exists(
            "this relationship already exists".into(),
        ));
    }
    let id = ids::new_id(ids::RELATIONSHIP);
    sqlx::query(
        "INSERT INTO relationships (id, book_id, from_entity_id, to_entity_id, kind, directed, description) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(book_id)
    .bind(&new.from_entity_id)
    .bind(&new.to_entity_id)
    .bind(&kind)
    .bind(i64::from(new.directed))
    .bind(&desc)
    .execute(pool)
    .await?;
    get_relationship(pool, &id).await
}

pub async fn get_relationship(pool: &SqlitePool, id: &str) -> Result<Relationship, StoreError> {
    check_id(id, ids::RELATIONSHIP, "relationship")?;
    let row: RelationshipRow = sqlx::query_as(concat!(
        "SELECT ",
        relationship_columns!(),
        " FROM relationships WHERE id = ?"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| StoreError::NotFound("relationship not found".into()))?;
    Ok(row.into())
}

/// Relationships of a book, or only those touching one entity.
pub async fn list_relationships(
    pool: &SqlitePool,
    book_id: &str,
    entity_id: Option<&str>,
) -> Result<Vec<Relationship>, StoreError> {
    require_book(pool, book_id).await?;
    let rows: Vec<RelationshipRow> = match entity_id {
        None => {
            sqlx::query_as(concat!(
                "SELECT ",
                relationship_columns!(),
                " FROM relationships WHERE book_id = ? ORDER BY created_at, id"
            ))
            .bind(book_id)
            .fetch_all(pool)
            .await?
        }
        Some(e) => {
            entity_kind_in_book(pool, e, book_id, "entity").await?;
            sqlx::query_as(concat!("SELECT ", relationship_columns!(), " FROM relationships \
                 WHERE book_id = ? AND (from_entity_id = ? OR to_entity_id = ?) ORDER BY created_at, id"))
            .bind(book_id)
            .bind(e)
            .bind(e)
            .fetch_all(pool)
            .await?
        }
    };
    Ok(rows.into_iter().map(Relationship::from).collect())
}

pub async fn update_relationship(
    pool: &SqlitePool,
    id: &str,
    patch: &RelationshipPatch,
) -> Result<Relationship, StoreError> {
    let current = get_relationship(pool, id).await?;
    if patch.kind.is_none() && patch.directed.is_none() && patch.description.is_none() {
        return Err(invalid("nothing to change"));
    }
    let kind = match &patch.kind {
        Some(k) => short_text("kind", k, MAX_KIND_CHARS)?,
        None => current.kind.clone(),
    };
    let directed = patch.directed.unwrap_or(current.directed);
    let desc = match &patch.description {
        Some(d) => description(d)?,
        None => current.description.clone(),
    };
    if relationship_exists(
        pool,
        &current.book_id,
        &current.from_entity_id,
        &current.to_entity_id,
        &kind,
        directed,
        Some(id),
    )
    .await?
    {
        return Err(StoreError::Exists(
            "this relationship already exists".into(),
        ));
    }
    sqlx::query(
        "UPDATE relationships SET kind = ?, directed = ?, description = ?, \
         updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
    )
    .bind(&kind)
    .bind(i64::from(directed))
    .bind(&desc)
    .bind(id)
    .execute(pool)
    .await?;
    get_relationship(pool, id).await
}

pub async fn delete_relationship(pool: &SqlitePool, id: &str) -> Result<(), StoreError> {
    get_relationship(pool, id).await?;
    sqlx::query("DELETE FROM relationships WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

// ---- timeline events -------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct EventRow {
    id: String,
    book_id: String,
    title: String,
    description: String,
    when_text: Option<String>,
    sort_order: i64,
    scene_id: Option<String>,
    place_id: Option<String>,
    created_at: String,
    updated_at: String,
}

impl EventRow {
    fn with(self, participants: Vec<Participant>) -> Event {
        Event {
            id: self.id,
            book_id: self.book_id,
            title: self.title,
            description: self.description,
            when_text: self.when_text,
            sort_order: self.sort_order,
            scene_id: self.scene_id,
            place_id: self.place_id,
            participants,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

macro_rules! event_columns {
    () => {
        "id, book_id, title, description, when_text, sort_order, scene_id, place_id, created_at, updated_at"
    };
}

fn clean_when(value: &Option<String>) -> Result<Option<String>, StoreError> {
    match value {
        None => Ok(None),
        Some(v) => {
            let c = manuscript::clean_field("when_text", v).map_err(invalid)?;
            Ok(if c.is_empty() { None } else { Some(c) })
        }
    }
}

async fn check_scene(pool: &SqlitePool, scene_id: &str, book_id: &str) -> Result<(), StoreError> {
    let row: Option<(String,)> = sqlx::query_as("SELECT book_id FROM scenes WHERE id = ?")
        .bind(scene_id)
        .fetch_optional(pool)
        .await?;
    match row {
        Some((b,)) if b == book_id => Ok(()),
        _ => Err(invalid("scene_id: not a scene of this book")),
    }
}

async fn check_place(pool: &SqlitePool, place_id: &str, book_id: &str) -> Result<(), StoreError> {
    if entity_kind_in_book(pool, place_id, book_id, "place_id").await? != "place" {
        return Err(invalid("place_id: the entity is not a place"));
    }
    Ok(())
}

async fn check_participants(
    pool: &SqlitePool,
    participants: &[Participant],
    book_id: &str,
) -> Result<Vec<Participant>, StoreError> {
    if participants.len() > MAX_PARTICIPANTS {
        return Err(invalid(format!("at most {MAX_PARTICIPANTS} participants")));
    }
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(participants.len());
    for p in participants {
        if !seen.insert(p.entity_id.as_str()) {
            return Err(invalid("an entity can take part in an event only once"));
        }
        entity_kind_in_book(pool, &p.entity_id, book_id, "participants").await?;
        let role = manuscript::clean_field("role", &p.role).map_err(invalid)?;
        if role.chars().count() > MAX_KIND_CHARS {
            return Err(invalid(format!(
                "role is longer than {MAX_KIND_CHARS} characters"
            )));
        }
        out.push(Participant {
            entity_id: p.entity_id.clone(),
            role,
        });
    }
    out.sort_by(|a, b| a.entity_id.cmp(&b.entity_id));
    Ok(out)
}

pub async fn create_event(
    pool: &SqlitePool,
    book_id: &str,
    new: &NewEvent,
) -> Result<Event, StoreError> {
    require_book(pool, book_id).await?;
    let title = short_text("title", &new.title, manuscript::MAX_FIELD_CHARS)?;
    let desc = description(&new.description)?;
    let when = clean_when(&new.when_text)?;
    if let Some(s) = &new.scene_id {
        check_scene(pool, s, book_id).await?;
    }
    if let Some(p) = &new.place_id {
        check_place(pool, p, book_id).await?;
    }
    let participants = check_participants(pool, &new.participants, book_id).await?;
    let id = ids::new_id(ids::EVENT);
    let mut tx = pool.begin().await?;
    let sort_order = match new.sort_order {
        Some(s) => s,
        None => {
            let (max,): (Option<i64>,) =
                sqlx::query_as("SELECT max(sort_order) FROM timeline_events WHERE book_id = ?")
                    .bind(book_id)
                    .fetch_one(&mut *tx)
                    .await?;
            max.map_or(10, |m| m.saturating_add(10))
        }
    };
    sqlx::query(
        "INSERT INTO timeline_events (id, book_id, title, description, when_text, sort_order, scene_id, place_id) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(book_id)
    .bind(&title)
    .bind(&desc)
    .bind(&when)
    .bind(sort_order)
    .bind(&new.scene_id)
    .bind(&new.place_id)
    .execute(&mut *tx)
    .await?;
    for p in &participants {
        sqlx::query(
            "INSERT INTO timeline_event_participants (event_id, entity_id, role) VALUES (?, ?, ?)",
        )
        .bind(&id)
        .bind(&p.entity_id)
        .bind(&p.role)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    get_event(pool, &id).await
}

async fn participants_of(
    pool: &SqlitePool,
    event_id: &str,
) -> Result<Vec<Participant>, StoreError> {
    Ok(sqlx::query(
        "SELECT entity_id, role FROM timeline_event_participants WHERE event_id = ? ORDER BY entity_id",
    )
    .bind(event_id)
    .fetch_all(pool)
    .await?
    .iter()
    .map(|r| Participant { entity_id: r.get("entity_id"), role: r.get("role") })
    .collect())
}

pub async fn get_event(pool: &SqlitePool, id: &str) -> Result<Event, StoreError> {
    check_id(id, ids::EVENT, "event")?;
    let row: EventRow = sqlx::query_as(concat!(
        "SELECT ",
        event_columns!(),
        " FROM timeline_events WHERE id = ?"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| StoreError::NotFound("event not found".into()))?;
    let participants = participants_of(pool, id).await?;
    Ok(row.with(participants))
}

/// Events of a book in timeline order, or only those an entity takes part in.
pub async fn list_events(
    pool: &SqlitePool,
    book_id: &str,
    entity_id: Option<&str>,
) -> Result<Vec<Event>, StoreError> {
    require_book(pool, book_id).await?;
    let rows: Vec<EventRow> = match entity_id {
        None => {
            sqlx::query_as(concat!(
                "SELECT ",
                event_columns!(),
                " FROM timeline_events WHERE book_id = ? ORDER BY sort_order, id"
            ))
            .bind(book_id)
            .fetch_all(pool)
            .await?
        }
        Some(e) => {
            entity_kind_in_book(pool, e, book_id, "entity").await?;
            sqlx::query_as(concat!("SELECT ", event_columns!(), " FROM timeline_events WHERE book_id = ? AND id IN \
                 (SELECT event_id FROM timeline_event_participants WHERE entity_id = ?) ORDER BY sort_order, id"))
            .bind(book_id)
            .bind(e)
            .fetch_all(pool)
            .await?
        }
    };
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let p = participants_of(pool, &r.id).await?;
        out.push(r.with(p));
    }
    Ok(out)
}

pub async fn update_event(
    pool: &SqlitePool,
    id: &str,
    patch: &EventPatch,
) -> Result<Event, StoreError> {
    let current = get_event(pool, id).await?;
    if patch.title.is_none()
        && patch.description.is_none()
        && patch.when_text.is_none()
        && patch.sort_order.is_none()
        && patch.scene_id.is_none()
        && patch.place_id.is_none()
        && patch.participants.is_none()
    {
        return Err(invalid("nothing to change"));
    }
    let title = match &patch.title {
        Some(t) => short_text("title", t, manuscript::MAX_FIELD_CHARS)?,
        None => current.title.clone(),
    };
    let desc = match &patch.description {
        Some(d) => description(d)?,
        None => current.description.clone(),
    };
    let when = match &patch.when_text {
        Some(w) => clean_when(w)?,
        None => current.when_text.clone(),
    };
    let scene_id = match &patch.scene_id {
        Some(Some(s)) => {
            check_scene(pool, s, &current.book_id).await?;
            Some(s.clone())
        }
        Some(None) => None,
        None => current.scene_id.clone(),
    };
    let place_id = match &patch.place_id {
        Some(Some(p)) => {
            check_place(pool, p, &current.book_id).await?;
            Some(p.clone())
        }
        Some(None) => None,
        None => current.place_id.clone(),
    };
    let participants = match &patch.participants {
        Some(p) => Some(check_participants(pool, p, &current.book_id).await?),
        None => None,
    };
    let mut tx = pool.begin().await?;
    sqlx::query(
        "UPDATE timeline_events SET title = ?, description = ?, when_text = ?, sort_order = ?, scene_id = ?, \
         place_id = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
    )
    .bind(&title)
    .bind(&desc)
    .bind(&when)
    .bind(patch.sort_order.unwrap_or(current.sort_order))
    .bind(&scene_id)
    .bind(&place_id)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    if let Some(list) = &participants {
        sqlx::query("DELETE FROM timeline_event_participants WHERE event_id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        for p in list {
            sqlx::query("INSERT INTO timeline_event_participants (event_id, entity_id, role) VALUES (?, ?, ?)")
                .bind(id)
                .bind(&p.entity_id)
                .bind(&p.role)
                .execute(&mut *tx)
                .await?;
        }
    }
    tx.commit().await?;
    get_event(pool, id).await
}

pub async fn delete_event(pool: &SqlitePool, id: &str) -> Result<(), StoreError> {
    get_event(pool, id).await?;
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM timeline_event_participants WHERE event_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM timeline_events WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}
