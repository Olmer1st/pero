//! `pero-server check-import DIR`: imports a folder of Markdown chapters into a throw-away store,
//! writes everything back out, and proves that no words were lost or changed.
//! Runs entirely offline and prints counts, file names and line numbers, never manuscript text.

use std::{fs, path::Path};

use crate::{
    db,
    import::{self, FileCheck, ImportFile, ParagraphMode},
    store::{Store, StoreError},
};

/// A paragraph spanning this many lines or more suggests the text uses one line per paragraph.
const LONG_PARAGRAPH_LINES: usize = 8;

#[derive(Debug, Default)]
pub struct Report {
    pub files: usize,
    pub chapters: usize,
    pub scenes: usize,
    pub passages: usize,
    pub titles_from_filename: usize,
    pub long_paragraphs: usize,
    /// Files whose words differ after the round trip. Must be empty.
    pub text_differs: Vec<String>,
    /// Files whose export is not byte-equal to the normalised original, with the first differing line.
    pub not_canonical: Vec<(String, Option<usize>)>,
}

impl Report {
    pub fn passed(&self) -> bool {
        self.text_differs.is_empty()
    }

    pub fn render(&self) -> String {
        let mut out = format!(
            "files: {}   chapters: {}   scenes: {}   passages: {}\n",
            self.files, self.chapters, self.scenes, self.passages
        );
        if self.titles_from_filename > 0 {
            out += &format!(
                "chapter titles made from file names (no '# ' heading): {}\n",
                self.titles_from_filename
            );
        }
        if self.long_paragraphs > 0 {
            out += &format!(
                "note: {} paragraphs span {LONG_PARAGRAPH_LINES} or more lines; if your text has one paragraph per line, run again with --mode line\n",
                self.long_paragraphs
            );
        }
        if self.passed() {
            out += "no text lost: PASS (every word, in order, comes back out)\n";
        } else {
            out += &format!(
                "no text lost: FAIL in {} file(s): {}\n",
                self.text_differs.len(),
                self.text_differs.join(", ")
            );
        }
        let canonical = self.files - self.not_canonical.len();
        out += &format!(
            "byte-identical to the normalised original: {canonical} of {} files\n",
            self.files
        );
        for (file, line) in self.not_canonical.iter().take(10) {
            match line {
                Some(l) => {
                    out += &format!("  differs only in notation, first at line {l}: {file}\n")
                }
                None => out += &format!("  differs only in notation: {file}\n"),
            }
        }
        out
    }
}

/// Reads the `*.md` files of a folder (not recursive).
pub fn read_folder(dir: &Path) -> Result<Vec<ImportFile>, String> {
    let mut files = Vec::new();
    let entries = fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !entry.file_type().map_err(|e| e.to_string())?.is_file()
            || !name.to_lowercase().ends_with(".md")
        {
            continue;
        }
        let content = fs::read_to_string(entry.path())
            .map_err(|e| format!("cannot read {name} as UTF-8 text: {e}"))?;
        files.push(ImportFile { name, content });
    }
    if files.is_empty() {
        return Err(format!("no .md files in {}", dir.display()));
    }
    Ok(files)
}

pub enum CheckError {
    /// The files were not understood; each line is `file:line: message`.
    Rejected(Vec<String>),
    Failed(String),
}

/// Imports into a temporary store, exports, and compares. The temporary folder is removed afterwards.
pub async fn run_check(
    files: Vec<ImportFile>,
    mode: ParagraphMode,
    language: &str,
) -> Result<Report, CheckError> {
    let dir = std::env::temp_dir().join(format!(
        "pero-check-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    ));
    let result = run_in(&dir, files, mode, language).await;
    let _ = fs::remove_dir_all(&dir);
    result
}

async fn run_in(
    dir: &Path,
    files: Vec<ImportFile>,
    mode: ParagraphMode,
    language: &str,
) -> Result<Report, CheckError> {
    let failed = |e: String| CheckError::Failed(e);
    fs::create_dir_all(dir).map_err(|e| failed(e.to_string()))?;
    let pool = db::open(&dir.join("check.db")).await.map_err(failed)?;
    let store = Store::new(pool, dir.join("books")).map_err(failed)?;
    let summary = match store
        .import_book("Import check", language, mode, &files, "check")
        .await
    {
        Ok(s) => s,
        Err(StoreError::ImportRejected(issues)) => {
            return Err(CheckError::Rejected(
                issues
                    .iter()
                    .map(|i| {
                        if i.line > 0 {
                            format!("{}:{}: {}", i.file, i.line, i.message)
                        } else {
                            format!("{}: {}", i.file, i.message)
                        }
                    })
                    .collect(),
            ));
        }
        Err(e) => return Err(failed(e.to_string())),
    };
    let exported = store
        .export_book(&summary.book.id)
        .await
        .map_err(|e| failed(e.to_string()))?;

    let mut originals = files;
    originals.sort_by(|a, b| import::natural_cmp(&a.name, &b.name));
    let mut report = Report {
        files: originals.len(),
        chapters: summary.chapters,
        scenes: summary.scenes,
        passages: summary.passages,
        titles_from_filename: summary.titles_from_filename.len(),
        ..Default::default()
    };
    if exported.len() != originals.len() {
        return Err(failed(
            "the export has a different number of chapters than the import".into(),
        ));
    }
    for (original, out) in originals.iter().zip(&exported) {
        let added = summary
            .titles_from_filename
            .contains(&original.name)
            .then_some(out.title.as_str());
        let FileCheck {
            same_text,
            canonical,
            first_difference_line,
            ..
        } = import::check_file(original, &out.content, added);
        if !same_text {
            report.text_differs.push(original.name.clone());
        }
        if !canonical {
            report
                .not_canonical
                .push((original.name.clone(), first_difference_line));
        }
    }
    if let Ok(chapters) = import::parse_files(&originals, mode) {
        report.long_paragraphs = chapters
            .iter()
            .flat_map(|c| &c.scenes)
            .flat_map(|s| &s.paragraphs)
            .filter(|p| p.lines().count() >= LONG_PARAGRAPH_LINES)
            .count();
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str, content: &str) -> ImportFile {
        ImportFile {
            name: name.into(),
            content: content.into(),
        }
    }

    #[tokio::test]
    async fn a_clean_folder_passes_and_prints_no_text() {
        let files = vec![
            file(
                "01.md",
                "# Первая\n\n## Утро\n\nСЕКРЕТНЫЙ абзац — ёж.\n\n* * *\n\nВторая сцена.\n",
            ),
            file(
                "02_vtoraya.md",
                "Без заголовка.\r\n\r\n\r\n***\r\n\r\nКонец.\r\n",
            ),
        ];
        let report = run_check(files, ParagraphMode::BlankLine, "ru")
            .await
            .ok()
            .unwrap();
        assert!(report.passed(), "{report:?}");
        assert_eq!(
            (
                report.files,
                report.chapters,
                report.scenes,
                report.passages
            ),
            (2, 2, 4, 4)
        );
        assert_eq!(report.titles_from_filename, 1);
        assert_eq!(report.not_canonical.len(), 0, "{report:?}");
        let text = report.render();
        assert!(text.contains("no text lost: PASS") && text.contains("2 of 2 files"));
        assert!(!text.contains("СЕКРЕТНЫЙ"));
    }

    #[tokio::test]
    async fn notation_differences_are_reported_without_failing() {
        let files = vec![file(
            "a.md",
            "# Глава\n\n***\n\nОдин.\n\n***\n\n***\n\nДва.\n",
        )];
        let report = run_check(files, ParagraphMode::BlankLine, "ru")
            .await
            .ok()
            .unwrap();
        assert!(report.passed());
        assert_eq!(report.not_canonical.len(), 1);
        assert!(report.render().contains("differs only in notation"));
    }

    #[tokio::test]
    async fn unreadable_structure_is_rejected_with_file_and_line() {
        let files = vec![file(
            "a.md",
            "# Глава\n\nтекст\n\n### Подзаголовок СЕКРЕТНЫЙ\n",
        )];
        match run_check(files, ParagraphMode::BlankLine, "ru").await {
            Err(CheckError::Rejected(lines)) => {
                assert_eq!(lines.len(), 1);
                assert!(
                    lines[0].starts_with("a.md:5:") && !lines[0].contains("СЕКРЕТНЫЙ"),
                    "{lines:?}"
                );
            }
            other => panic!("expected a rejection, got {:?}", other.is_ok()),
        }
    }

    #[tokio::test]
    async fn single_line_paragraphs_are_noticed() {
        let long: String = (0..10).map(|i| format!("Реплика {i}.\n")).collect();
        let files = vec![file("a.md", &format!("# Г\n\n{long}"))];
        let report = run_check(files, ParagraphMode::BlankLine, "ru")
            .await
            .ok()
            .unwrap();
        assert_eq!(report.long_paragraphs, 1);
        assert!(report.render().contains("--mode line"));
    }

    #[test]
    fn folders_are_read_by_extension_and_empty_ones_are_refused() {
        let dir = std::env::temp_dir().join(format!("pero-check-read-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("a.md"), "# А\n").unwrap();
        fs::write(dir.join("notes.txt"), "x").unwrap();
        let files = read_folder(&dir).unwrap();
        assert_eq!(files.len(), 1);
        fs::remove_file(dir.join("a.md")).unwrap();
        assert!(read_folder(&dir).is_err());
        let _ = fs::remove_dir_all(&dir);
    }
}
