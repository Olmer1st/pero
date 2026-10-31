use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::double_option;

#[derive(TS, Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EvidenceOut {
    pub passage_id: String,
    /// `active`, or `removed` when the cited passage no longer exists in the text.
    pub passage_status: String,
    /// True when the passage text changed since it was cited.
    pub changed: bool,
}

#[derive(TS, Debug, Clone, Serialize, PartialEq, Eq)]
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

#[derive(TS, Debug, Clone, Serialize, PartialEq, Eq)]
pub struct HistoryEntry {
    pub from_status: Option<String>,
    pub to_status: String,
    pub changed_by: String,
    pub reason: Option<String>,
    pub changed_at: String,
}

#[derive(TS, Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PassageInfo {
    pub id: String,
    pub book_id: String,
    pub scene_id: String,
    #[ts(type = "number")]
    pub position: i64,
    pub status: String,
    pub merged_into: Option<String>,
    pub removed_at: Option<String>,
}

#[derive(TS, Debug, Clone, Deserialize)]
#[ts(optional_fields)]
#[serde(deny_unknown_fields)]
pub struct EvidenceIn {
    pub passage_id: String,
}

/// `status` is optional: the author's facts default to `author_approved`, AI facts are always
/// `ai_hypothesis`. Approval fields cannot be sent; only `approve` writes them.
#[derive(TS, Debug, Deserialize)]
#[ts(optional_fields)]
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
    #[ts(optional, as = "Option<Vec<EvidenceIn>>")]
    pub evidence: Vec<EvidenceIn>,
}

/// Text and subject can be edited; the status can only change through `approve`, `reject`, `reopen`.
#[derive(TS, Debug, Deserialize)]
#[ts(optional_fields)]
#[serde(deny_unknown_fields)]
pub struct FactPatch {
    pub text: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    pub entity_id: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub source_ref: Option<Option<String>>,
}

#[derive(TS, Debug, Default, Deserialize)]
#[ts(optional_fields)]
#[serde(deny_unknown_fields)]
pub struct TransitionBody {
    pub reason: Option<String>,
}

#[derive(TS, Debug, Deserialize)]
#[ts(optional_fields)]
#[serde(deny_unknown_fields)]
pub struct EvidenceReplace {
    pub evidence: Vec<EvidenceIn>,
}

/// Answer of `GET /facts/{id}/history`: oldest first.
#[derive(TS, Debug, Serialize)]
pub struct HistoryResponse {
    pub history: Vec<HistoryEntry>,
}
