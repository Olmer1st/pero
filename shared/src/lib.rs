//! Pero API types, defined once. The server (`core/`) serialises and parses exactly these types,
//! and `client/src/generated/api.ts` is generated from them (see `typescript.rs`), so the two sides
//! cannot drift: a changed type fails the test `generated_types_are_current` until the file is
//! regenerated and committed.

pub mod canon;
pub mod device;
pub mod error;
pub mod graph;
pub mod health;
pub mod manuscript;
#[cfg(test)]
mod typescript;

use serde::{Deserialize, Deserializer};

/// For patch bodies where an absent field means "leave as is" and `null` means "clear".
pub fn double_option<'de, T, D>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Deserialize::deserialize(d).map(Some)
}

/// Closed sets that the API accepts or returns as plain strings. The server validates against
/// these; they are also generated into the client as literal types.
pub const ENTITY_KINDS: [&str; 3] = ["character", "place", "organization"];
pub const FACT_STATUSES: [&str; 4] = ["explicit", "author_approved", "ai_hypothesis", "rejected"];
pub const FACT_ORIGINS: [&str; 3] = ["author", "ai", "import"];
/// A cited passage is `active`, or `removed` when it no longer exists in the text.
pub const PASSAGE_STATUSES: [&str; 2] = ["active", "removed"];
