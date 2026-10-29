//! Generates `client/src/generated/api.ts` from the Rust types. The committed file is checked by a
//! test, so a type changed in Rust fails the build until the file is regenerated:
//!
//! ```text
//! PERO_UPDATE_TYPES=1 cargo test -p pero-shared generated_types_are_current
//! ```

use ts_rs::{Config, TS};

use crate::{
    ENTITY_KINDS, FACT_ORIGINS, FACT_STATUSES, PASSAGE_STATUSES, canon::*, device::*, error::*,
    graph::*, health::*, manuscript::*,
};

fn constant(name: &str, type_name: &str, values: &[&str]) -> String {
    let list = values
        .iter()
        .map(|v| format!("\"{v}\""))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "export const {name} = [{list}] as const;\nexport type {type_name} = (typeof {name})[number];\n"
    )
}

/// The whole generated file.
pub fn generate() -> String {
    let mut out = String::from(
        "// GENERATED from the Rust types in `shared/` by `cargo test -p pero-shared`. Do not edit.\n\
         // Regenerate: PERO_UPDATE_TYPES=1 cargo test -p pero-shared generated_types_are_current\n\n",
    );
    let cfg = Config::new();
    macro_rules! decls {
        ($($t:ty),* $(,)?) => { $( out.push_str("export "); out.push_str(&<$t as TS>::decl(&cfg)); out.push_str("\n\n"); )* };
    }
    decls!(
        Book,
        Chapter,
        SceneInfo,
        PassageOut,
        Scene,
        SceneFields,
        PassageIn,
        Merge,
        SaveScene,
        Saved,
        ImportSummary,
        ExportFile,
        ExportResponse,
        ReindexResponse,
        ParagraphMode,
        ImportFile,
        ImportRequest,
        NewBook,
        TitleBody,
        NewChapter,
        NewScene,
        OrderBody,
        Entity,
        NewEntity,
        EntityPatch,
        Relationship,
        NewRelationship,
        RelationshipPatch,
        Participant,
        Event,
        NewEvent,
        EventPatch,
        EvidenceOut,
        Fact,
        HistoryEntry,
        HistoryResponse,
        PassageInfo,
        EvidenceIn,
        NewFact,
        FactPatch,
        TransitionBody,
        EvidenceReplace,
        Device,
        DeviceInfo,
        ServiceStatus,
        Services,
        HealthReport,
        ImportIssue,
        ErrorBody,
        ErrorDetail,
    );
    out.push_str(&constant("ENTITY_KINDS", "EntityKind", &ENTITY_KINDS));
    out.push('\n');
    out.push_str(&constant("FACT_STATUSES", "FactStatus", &FACT_STATUSES));
    out.push('\n');
    out.push_str(&constant("FACT_ORIGINS", "FactOrigin", &FACT_ORIGINS));
    out.push('\n');
    out.push_str(&constant(
        "PASSAGE_STATUSES",
        "PassageStatus",
        &PASSAGE_STATUSES,
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::generate;

    fn path() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../client/src/generated/api.ts")
    }

    #[test]
    fn generated_types_are_current() {
        let fresh = generate();
        if std::env::var_os("PERO_UPDATE_TYPES").is_some() {
            std::fs::create_dir_all(path().parent().unwrap()).unwrap();
            std::fs::write(path(), &fresh).unwrap();
            return;
        }
        let committed = std::fs::read_to_string(path()).unwrap_or_default();
        assert!(
            committed == fresh,
            "client/src/generated/api.ts is out of date with the Rust types in shared/.\n\
             Regenerate it and commit it: PERO_UPDATE_TYPES=1 cargo test -p pero-shared generated_types_are_current"
        );
    }
}
