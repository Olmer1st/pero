use serde::Serialize;
use ts_rs::TS;

use crate::manuscript::Scene;

/// A problem found while importing a file.
#[derive(TS, Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImportIssue {
    pub file: String,
    /// 1-based line, or 0 when the problem concerns the whole file.
    pub line: usize,
    pub message: String,
}

/// Every error answer of the API (`/api/v1`) has this shape.
#[derive(TS, Debug, Serialize)]
pub struct ErrorBody {
    pub error: ErrorDetail,
    /// Only for `import_rejected`: what was wrong and where (never the text itself).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub issues: Option<Vec<ImportIssue>>,
    /// Only for `revision_conflict`: the scene as it is now.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub current: Option<Box<Scene>>,
}

#[derive(TS, Debug, Serialize)]
pub struct ErrorDetail {
    /// Stable machine-readable code, for example `not_found`, `invalid`, `revision_conflict`.
    pub code: String,
    pub message: String,
}
