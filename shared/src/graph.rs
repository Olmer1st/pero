use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::double_option;

#[derive(TS, Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Entity {
    pub id: String,
    pub book_id: String,
    pub kind: String,
    pub name: String,
    pub description: String,
    /// Other names and forms, sorted.
    pub aliases: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(TS, Debug, Deserialize)]
#[ts(optional_fields)]
#[serde(deny_unknown_fields)]
pub struct NewEntity {
    pub kind: String,
    pub name: String,
    #[serde(default)]
    #[ts(optional, as = "Option<String>")]
    pub description: String,
    #[serde(default)]
    #[ts(optional, as = "Option<Vec<String>>")]
    pub aliases: Vec<String>,
}

/// The kind of an entity cannot change; unknown fields (including `kind`) are refused.
#[derive(TS, Debug, Deserialize)]
#[ts(optional_fields)]
#[serde(deny_unknown_fields)]
pub struct EntityPatch {
    pub name: Option<String>,
    pub description: Option<String>,
    /// Replaces all aliases when present.
    pub aliases: Option<Vec<String>>,
}

#[derive(TS, Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Relationship {
    pub id: String,
    pub book_id: String,
    pub from_entity_id: String,
    pub to_entity_id: String,
    pub kind: String,
    pub directed: bool,
    pub description: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(TS, Debug, Deserialize)]
#[ts(optional_fields)]
#[serde(deny_unknown_fields)]
pub struct NewRelationship {
    pub from_entity_id: String,
    pub to_entity_id: String,
    pub kind: String,
    #[serde(default = "yes")]
    #[ts(optional, as = "Option<bool>")]
    pub directed: bool,
    #[serde(default)]
    #[ts(optional, as = "Option<String>")]
    pub description: String,
}

/// The two entities of a relationship cannot change; delete and create a new one instead.
#[derive(TS, Debug, Deserialize)]
#[ts(optional_fields)]
#[serde(deny_unknown_fields)]
pub struct RelationshipPatch {
    pub kind: Option<String>,
    pub directed: Option<bool>,
    pub description: Option<String>,
}

#[derive(TS, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Participant {
    pub entity_id: String,
    #[serde(default)]
    #[ts(optional, as = "Option<String>")]
    pub role: String,
}

#[derive(TS, Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Event {
    pub id: String,
    pub book_id: String,
    pub title: String,
    pub description: String,
    /// The date as the author writes it (fictional calendars are fine).
    pub when_text: Option<String>,
    /// Author-controlled order on the timeline; lower comes first.
    #[ts(type = "number")]
    pub sort_order: i64,
    pub scene_id: Option<String>,
    pub place_id: Option<String>,
    pub participants: Vec<Participant>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(TS, Debug, Deserialize)]
#[ts(optional_fields)]
#[serde(deny_unknown_fields)]
pub struct NewEvent {
    pub title: String,
    #[serde(default)]
    #[ts(optional, as = "Option<String>")]
    pub description: String,
    pub when_text: Option<String>,
    /// Defaults to 10 after the last event of the book.
    #[ts(optional, type = "number")]
    pub sort_order: Option<i64>,
    pub scene_id: Option<String>,
    pub place_id: Option<String>,
    #[serde(default)]
    #[ts(optional, as = "Option<Vec<Participant>>")]
    pub participants: Vec<Participant>,
}

#[derive(TS, Debug, Deserialize)]
#[ts(optional_fields)]
#[serde(deny_unknown_fields)]
pub struct EventPatch {
    pub title: Option<String>,
    pub description: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    pub when_text: Option<Option<String>>,
    #[ts(optional, type = "number")]
    pub sort_order: Option<i64>,
    #[serde(default, deserialize_with = "double_option")]
    pub scene_id: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub place_id: Option<Option<String>>,
    /// Replaces all participants when present.
    pub participants: Option<Vec<Participant>>,
}

pub fn yes() -> bool {
    true
}
