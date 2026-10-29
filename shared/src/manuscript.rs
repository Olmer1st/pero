use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(TS, Deserialize)]
#[ts(optional_fields)]
#[serde(deny_unknown_fields)]
pub struct NewBook {
    pub title: String,
    pub language: String,
}

#[derive(TS, Deserialize)]
#[ts(optional_fields)]
#[serde(deny_unknown_fields)]
pub struct ImportRequest {
    pub title: String,
    pub language: String,
    #[serde(default)]
    #[ts(optional, as = "Option<ParagraphMode>")]
    pub mode: ParagraphMode,
    pub files: Vec<ImportFile>,
}

#[derive(TS, Deserialize)]
#[ts(optional_fields)]
#[serde(deny_unknown_fields)]
pub struct TitleBody {
    pub title: String,
}

#[derive(TS, Deserialize)]
#[ts(optional_fields)]
#[serde(deny_unknown_fields)]
pub struct NewChapter {
    pub title: String,
    pub position: Option<usize>,
}

#[derive(TS, Deserialize)]
#[ts(optional_fields)]
#[serde(deny_unknown_fields)]
pub struct NewScene {
    #[serde(default)]
    #[ts(optional, as = "Option<String>")]
    pub title: String,
    pub language: Option<String>,
    pub pov: Option<String>,
    pub date: Option<String>,
    pub location: Option<String>,
    pub position: Option<usize>,
}

#[derive(TS, Deserialize)]
#[ts(optional_fields)]
#[serde(deny_unknown_fields)]
pub struct OrderBody {
    pub ids: Vec<String>,
}

#[derive(TS, Debug, Clone, Serialize)]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct Book {
    pub id: String,
    pub title: String,
    pub language: String,
    pub head_commit: Option<String>,
}

#[derive(TS, Debug, Clone, Serialize)]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct Chapter {
    pub id: String,
    pub book_id: String,
    #[ts(type = "number")]
    pub position: i64,
    pub title: String,
}

#[derive(TS, Debug, Clone, Serialize)]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct SceneInfo {
    pub id: String,
    pub book_id: String,
    pub chapter_id: String,
    #[ts(type = "number")]
    pub position: i64,
    pub title: String,
    pub language: String,
    pub pov: Option<String>,
    pub date: Option<String>,
    pub location: Option<String>,
}

#[derive(TS, Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PassageOut {
    /// `None` for a paragraph written outside Pero that has no marker yet.
    pub id: Option<String>,
    pub text: String,
}

#[derive(TS, Debug, Clone, Serialize)]
pub struct Scene {
    #[serde(flatten)]
    pub info: SceneInfo,
    pub passages: Vec<PassageOut>,
    /// SHA-256 of the scene file; send it back as `base_revision` when saving.
    pub revision: String,
}

/// Scene metadata as the client sends it. Saving replaces all of it.
#[derive(TS, Debug, Clone, Default, Deserialize)]
#[ts(optional_fields)]
#[serde(deny_unknown_fields)]
pub struct SceneFields {
    #[serde(default)]
    #[ts(optional, as = "Option<String>")]
    pub title: String,
    pub language: Option<String>,
    pub pov: Option<String>,
    pub date: Option<String>,
    pub location: Option<String>,
}

#[derive(TS, Debug, Clone, Deserialize)]
#[ts(optional_fields)]
#[serde(deny_unknown_fields)]
pub struct PassageIn {
    /// Existing passage ID, or absent for a new paragraph.
    pub id: Option<String>,
    pub text: String,
}

/// A passage that was merged into another one (its identity moves there for evidence purposes).
#[derive(TS, Debug, Clone, Deserialize)]
#[ts(optional_fields)]
#[serde(deny_unknown_fields)]
pub struct Merge {
    pub from: String,
    pub into: String,
}

#[derive(TS, Debug, Clone, Deserialize)]
#[ts(optional_fields)]
#[serde(deny_unknown_fields)]
pub struct SaveScene {
    pub base_revision: String,
    pub fields: SceneFields,
    pub passages: Vec<PassageIn>,
    #[serde(default)]
    #[ts(optional, as = "Option<Vec<Merge>>")]
    pub merged: Vec<Merge>,
}

#[derive(TS, Debug, Serialize)]
pub struct Saved {
    pub scene: Scene,
    /// The new commit, or `None` when the save changed nothing.
    pub commit: Option<String>,
}

#[derive(TS, Debug, Serialize)]
pub struct ImportSummary {
    pub book: Book,
    pub chapters: usize,
    pub scenes: usize,
    pub passages: usize,
    /// Files that had no `# ` heading, so the chapter title was made from the file name.
    pub titles_from_filename: Vec<String>,
}

/// A chapter written back out as Markdown.
#[derive(TS, Debug, Serialize)]
pub struct ExportFile {
    pub name: String,
    pub content: String,
    /// True when the chapter title is the one the import made up from a file name.
    #[serde(skip)]
    pub title: String,
}

#[derive(TS, Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParagraphMode {
    /// Paragraphs are separated by blank lines (usual Markdown).
    #[default]
    BlankLine,
    /// Every non-empty line is its own paragraph (text exported from word processors).
    Line,
}

#[derive(TS, Debug, Clone, Deserialize)]
#[ts(optional_fields)]
#[serde(deny_unknown_fields)]
pub struct ImportFile {
    /// File name only, ending in `.md`.
    pub name: String,
    pub content: String,
}

/// Answer of `GET /books/{id}/export`.
#[derive(TS, Debug, Serialize)]
pub struct ExportResponse {
    pub files: Vec<ExportFile>,
}

/// Answer of `POST /books/{id}/reindex`.
#[derive(TS, Debug, Serialize)]
pub struct ReindexResponse {
    /// Paragraphs written outside Pero that still have no passage marker.
    pub unmarked_paragraphs: usize,
}
