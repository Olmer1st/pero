//! Reading a folder of Markdown chapters into chapters, scenes and passages, writing them back out
//! as Markdown, and checking that nothing was lost on the way. Pure functions: no files, no database.
//!
//! Input rules (one file = one chapter):
//!   `# Title`           the chapter title, first in the file (else taken from the file name)
//!   `## Title`          starts a scene with that title
//!   `* * *`, `***`, `---`, `___`   a scene break (starts an untitled scene)
//!   anything else       paragraphs; with `ParagraphMode::BlankLine` a paragraph is a block of lines
//!                       between blank lines, with `ParagraphMode::Line` every non-empty line is one
//! Other headings (`###`, `#text`) are refused rather than guessed at.
//! Messages never quote the manuscript: they name the file and the line.

use serde::Deserialize;
use std::cmp::Ordering;

use crate::manuscript;

const MAX_ISSUES: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParagraphMode {
    /// Paragraphs are separated by blank lines (usual Markdown).
    #[default]
    BlankLine,
    /// Every non-empty line is its own paragraph (text exported from word processors).
    Line,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportFile {
    /// File name only, ending in `.md`.
    pub name: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub file: String,
    /// 1-based line, or 0 when the problem concerns the whole file.
    pub line: usize,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedScene {
    pub title: String,
    pub paragraphs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedChapter {
    pub file: String,
    pub title: String,
    pub title_from_filename: bool,
    pub scenes: Vec<ParsedScene>,
}

impl ParsedChapter {
    #[cfg(test)]
    pub fn passage_count(&self) -> usize {
        self.scenes.iter().map(|s| s.paragraphs.len()).sum()
    }
}

/// Orders names the way people expect: `2.md` before `10.md`.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    fn chunks(s: &str) -> Vec<(bool, String)> {
        let mut out: Vec<(bool, String)> = Vec::new();
        for c in s.chars() {
            let digit = c.is_ascii_digit();
            match out.last_mut() {
                Some((d, run)) if *d == digit => run.push(c),
                _ => out.push((digit, c.to_string())),
            }
        }
        out
    }
    let (ca, cb) = (chunks(a), chunks(b));
    for ((da, ra), (db, rb)) in ca.iter().zip(&cb) {
        let ord = if *da && *db {
            let (ta, tb) = (ra.trim_start_matches('0'), rb.trim_start_matches('0'));
            ta.len()
                .cmp(&tb.len())
                .then_with(|| ta.cmp(tb))
                .then_with(|| ra.len().cmp(&rb.len()))
        } else {
            ra.cmp(rb)
        };
        if ord != Ordering::Equal {
            return ord;
        }
    }
    ca.len().cmp(&cb.len())
}

fn unify(content: &str) -> String {
    content
        .strip_prefix('\u{feff}')
        .unwrap_or(content)
        .replace("\r\n", "\n")
        .replace('\r', "\n")
}

fn is_scene_break(line: &str) -> bool {
    let compact: String = line.chars().filter(|c| !c.is_whitespace()).collect();
    compact.len() >= 3
        && ["*", "-", "_"]
            .iter()
            .any(|ch| compact.chars().all(|c| ch.starts_with(c)))
}

#[derive(Debug, PartialEq, Eq)]
enum Block {
    Heading1(String),
    Heading2(String),
    Break,
    Paragraph(String),
}

/// Splits a chapter file into blocks with their first line number.
fn scan(
    file: &str,
    content: &str,
    mode: ParagraphMode,
    issues: &mut Vec<Issue>,
) -> Vec<(usize, Block)> {
    let text = unify(content);
    let mut raw: Vec<(usize, Vec<&str>)> = Vec::new();
    let mut open = false;
    for (i, line) in text.split('\n').enumerate() {
        if line.trim().is_empty() {
            open = false;
        } else if open && mode == ParagraphMode::BlankLine {
            raw.last_mut().expect("open block").1.push(line);
        } else {
            raw.push((i + 1, vec![line]));
            open = true;
        }
    }
    let mut push_issue = |line: usize, message: &str| {
        issues.push(Issue {
            file: file.to_string(),
            line,
            message: message.to_string(),
        });
    };
    let mut blocks = Vec::new();
    for (line_no, lines) in raw {
        let first = lines[0].trim();
        if lines.len() == 1 && is_scene_break(first) {
            blocks.push((line_no, Block::Break));
        } else if lines
            .iter()
            .any(|l| l.trim_start().starts_with('#') || is_scene_break(l.trim()))
        {
            if lines.len() > 1 {
                push_issue(
                    line_no,
                    "a heading or scene break line must be separated from the text by blank lines",
                );
            } else if let Some(h) = first.strip_prefix("## ") {
                blocks.push((line_no, Block::Heading2(h.trim().to_string())));
            } else if let Some(h) = first.strip_prefix("# ") {
                blocks.push((line_no, Block::Heading1(h.trim().to_string())));
            } else {
                push_issue(
                    line_no,
                    "unsupported heading (only '# ' for the chapter and '## ' for a scene)",
                );
            }
        } else {
            blocks.push((line_no, Block::Paragraph(lines.join("\n"))));
        }
    }
    blocks
}

fn title_from_filename(name: &str, number: usize) -> String {
    let stem = name.rsplit_once('.').map_or(name, |(s, _)| s);
    let stem =
        stem.trim_start_matches(|c: char| c.is_ascii_digit() || matches!(c, ' ' | '.' | '_' | '-'));
    let title = stem.replace(['_', '-'], " ");
    let title = title.trim();
    if title.is_empty() {
        format!("Chapter {number}")
    } else {
        title.to_string()
    }
}

fn clean_title(file: &str, line: usize, text: &str, issues: &mut Vec<Issue>) -> String {
    match manuscript::clean_field("title", text) {
        Ok(t) if !t.is_empty() => t,
        Ok(_) => {
            issues.push(Issue {
                file: file.into(),
                line,
                message: "the heading is empty".into(),
            });
            String::new()
        }
        Err(e) => {
            issues.push(Issue {
                file: file.into(),
                line,
                message: e,
            });
            String::new()
        }
    }
}

fn parse_chapter(
    file: &ImportFile,
    number: usize,
    mode: ParagraphMode,
    issues: &mut Vec<Issue>,
) -> ParsedChapter {
    let mut title: Option<String> = None;
    let mut seen_content = false;
    let mut scenes: Vec<ParsedScene> = Vec::new();
    let mut cur = ParsedScene {
        title: String::new(),
        paragraphs: Vec::new(),
    };
    for (line, block) in scan(&file.name, &file.content, mode, issues) {
        match block {
            Block::Heading1(h) => {
                if title.is_none() && !seen_content {
                    title = Some(clean_title(&file.name, line, &h, issues));
                } else {
                    issues.push(Issue {
                        file: file.name.clone(),
                        line,
                        message: "a second '# ' heading in one chapter file".into(),
                    });
                }
            }
            Block::Heading2(h) => {
                seen_content = true;
                let t = clean_title(&file.name, line, &h, issues);
                if cur.paragraphs.is_empty() && cur.title.is_empty() {
                    cur.title = t;
                } else {
                    scenes.push(std::mem::replace(
                        &mut cur,
                        ParsedScene {
                            title: t,
                            paragraphs: Vec::new(),
                        },
                    ));
                }
            }
            Block::Break => {
                seen_content = true;
                if !cur.paragraphs.is_empty() {
                    scenes.push(std::mem::replace(
                        &mut cur,
                        ParsedScene {
                            title: String::new(),
                            paragraphs: Vec::new(),
                        },
                    ));
                }
            }
            Block::Paragraph(text) => {
                seen_content = true;
                match manuscript::normalize_paragraph(&text) {
                    Ok(p) => cur.paragraphs.push(p),
                    Err(e) => issues.push(Issue {
                        file: file.name.clone(),
                        line,
                        message: e,
                    }),
                }
            }
        }
    }
    if !cur.paragraphs.is_empty() || !cur.title.is_empty() {
        scenes.push(cur);
    }
    let from_name = title.is_none();
    ParsedChapter {
        file: file.name.clone(),
        title: title.unwrap_or_else(|| title_from_filename(&file.name, number)),
        title_from_filename: from_name,
        scenes,
    }
}

/// Parses all files (in natural file-name order). Either every file is understood or the whole
/// import is refused with a list of problems (at most 50), each naming a file and a line.
pub fn parse_files(
    files: &[ImportFile],
    mode: ParagraphMode,
) -> Result<Vec<ParsedChapter>, Vec<Issue>> {
    let mut issues = Vec::new();
    if files.is_empty() {
        return Err(vec![Issue {
            file: String::new(),
            line: 0,
            message: "no files to import".into(),
        }]);
    }
    let mut sorted: Vec<&ImportFile> = files.iter().collect();
    sorted.sort_by(|a, b| natural_cmp(&a.name, &b.name));
    let mut names = std::collections::HashSet::new();
    for f in &sorted {
        let ok_name = !f.name.is_empty()
            && !f.name.contains(['/', '\\', '\0'])
            && !f.name.starts_with('.')
            && f.name.to_lowercase().ends_with(".md");
        if !ok_name {
            issues.push(Issue {
                file: f.name.clone(),
                line: 0,
                message: "not a plain file name ending in .md".into(),
            });
        } else if !names.insert(f.name.as_str()) {
            issues.push(Issue {
                file: f.name.clone(),
                line: 0,
                message: "duplicate file name".into(),
            });
        }
    }
    if sorted.len() > 999 {
        issues.push(Issue {
            file: String::new(),
            line: 0,
            message: "more than 999 chapter files".into(),
        });
    }
    let mut chapters = Vec::new();
    for (i, f) in sorted.iter().enumerate() {
        let chapter = parse_chapter(f, i + 1, mode, &mut issues);
        if chapter.scenes.len() > 999 {
            issues.push(Issue {
                file: f.name.clone(),
                line: 0,
                message: "more than 999 scenes in one chapter".into(),
            });
        }
        chapters.push(chapter);
    }
    if issues.is_empty() {
        Ok(chapters)
    } else {
        issues.truncate(MAX_ISSUES);
        Err(issues)
    }
}

/// Canonical Markdown of one chapter: the form `normalize_markdown` produces for a well-formed file.
pub fn export_chapter(title: &str, scenes: &[(String, Vec<String>)]) -> String {
    let mut out = format!("# {title}\n\n");
    for (i, (scene_title, paragraphs)) in scenes.iter().enumerate() {
        if !scene_title.is_empty() {
            out.push_str(&format!("## {scene_title}\n\n"));
        } else if i > 0 {
            out.push_str("* * *\n\n");
        }
        for p in paragraphs {
            out.push_str(p);
            out.push_str("\n\n");
        }
    }
    format!("{}\n", out.trim_end())
}

/// Only whitespace and notation are normalised: line endings, trailing spaces, runs of blank lines,
/// the spelling of scene breaks (to `* * *`) and the final newline. Words and punctuation are untouched.
pub fn normalize_markdown(content: &str) -> String {
    let text = unify(content);
    let mut out: Vec<String> = Vec::new();
    for line in text.split('\n') {
        let line = line.trim_end();
        let line = if is_scene_break(line.trim()) {
            "* * *"
        } else {
            line
        };
        if line.is_empty() && out.last().is_none_or(|l| l.is_empty()) {
            continue;
        }
        out.push(line.to_string());
    }
    while out.last().is_some_and(|l| l.is_empty()) {
        out.pop();
    }
    format!("{}\n", out.join("\n"))
}

/// The characters of a Markdown text that are not whitespace, scene-break lines or heading marks.
/// Two texts with the same fingerprint contain the same words in the same order.
pub fn fingerprint(content: &str) -> String {
    let mut out = String::new();
    for line in unify(content).split('\n') {
        let line = line.trim();
        if is_scene_break(line) {
            continue;
        }
        let line = if line.starts_with('#') {
            line.trim_start_matches('#')
        } else {
            line
        };
        out.extend(line.chars().filter(|c| !c.is_whitespace()));
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileCheck {
    pub file: String,
    /// Same words in the same order (see `fingerprint`). This is the "no text loss" check.
    pub same_text: bool,
    /// Byte-equal to the canonical form after `normalize_markdown`.
    pub canonical: bool,
    /// First line (1-based) where the normalised original and the export differ, when they do.
    pub first_difference_line: Option<usize>,
}

/// Compares an original chapter file with what was exported back out of the store.
/// `added_title` is the heading that the import made up from the file name, if any.
pub fn check_file(original: &ImportFile, exported: &str, added_title: Option<&str>) -> FileCheck {
    let mut expected = normalize_markdown(&original.content);
    if let Some(t) = added_title {
        expected = format!("# {t}\n\n{expected}");
    }
    let mut original_fingerprint = fingerprint(&original.content);
    if let Some(t) = added_title {
        original_fingerprint = fingerprint(&format!("# {t}")) + &original_fingerprint;
    }
    let same_text = original_fingerprint == fingerprint(exported);
    let canonical = expected == exported;
    let first_difference_line = (!canonical).then(|| {
        expected
            .lines()
            .zip(exported.lines())
            .position(|(a, b)| a != b)
            .map_or(
                expected.lines().count().min(exported.lines().count()) + 1,
                |i| i + 1,
            )
    });
    FileCheck {
        file: original.name.clone(),
        same_text,
        canonical,
        first_difference_line,
    }
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

    fn scenes_of(c: &ParsedChapter) -> Vec<(String, Vec<String>)> {
        c.scenes
            .iter()
            .map(|s| (s.title.clone(), s.paragraphs.clone()))
            .collect()
    }

    const CANONICAL: &str = "# Глава первая\n\n## Утро\n\nНад Доном вставало солнце — холодное, чужое.\n\n— Ну что, казак, — сказал он, — поедем?\n\n* * *\n\n«Ёлочки» и ёж.\nВторая строка абзаца.\n\n## Вечер\n\n„Лапки“ и… многоточие.\n";

    fn issues(files: &[ImportFile]) -> Vec<Issue> {
        parse_files(files, ParagraphMode::BlankLine).unwrap_err()
    }

    #[test]
    fn natural_order_puts_2_before_10() {
        let mut names = vec!["10-b.md", "2-a.md", "02-c.md", "1.md", "a.md"];
        names.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(names, vec!["1.md", "2-a.md", "02-c.md", "10-b.md", "a.md"]);
    }

    #[test]
    fn a_well_formed_chapter_is_split_into_scenes_and_paragraphs() {
        let chapters = parse_files(&[file("01.md", CANONICAL)], ParagraphMode::BlankLine).unwrap();
        let c = &chapters[0];
        assert_eq!(
            (c.title.as_str(), c.title_from_filename),
            ("Глава первая", false)
        );
        assert_eq!(c.scenes.len(), 3);
        assert_eq!(c.scenes[0].title, "Утро");
        assert_eq!(
            c.scenes[0].paragraphs,
            [
                "Над Доном вставало солнце — холодное, чужое.",
                "— Ну что, казак, — сказал он, — поедем?"
            ]
        );
        assert_eq!(c.scenes[1].title, "");
        assert_eq!(
            c.scenes[1].paragraphs,
            ["«Ёлочки» и ёж.\nВторая строка абзаца."]
        );
        assert_eq!(c.scenes[2].title, "Вечер");
        assert_eq!(c.passage_count(), 4);
    }

    #[test]
    fn export_gives_back_the_canonical_text_exactly() {
        let c = parse_files(&[file("01.md", CANONICAL)], ParagraphMode::BlankLine)
            .unwrap()
            .remove(0);
        let exported = export_chapter(&c.title, &scenes_of(&c));
        assert_eq!(exported, CANONICAL);
        assert_eq!(normalize_markdown(CANONICAL), CANONICAL);
        let check = check_file(&file("01.md", CANONICAL), &exported, None);
        assert_eq!(
            check,
            FileCheck {
                file: "01.md".into(),
                same_text: true,
                canonical: true,
                first_difference_line: None
            }
        );
    }

    #[test]
    fn messy_whitespace_and_break_spellings_normalise_to_the_same_text() {
        let messy = "\u{feff}# Глава первая  \r\n\r\n\r\n## Утро\r\n\r\nНад Доном вставало солнце — холодное, чужое.   \r\n\r\n\r\n— Ну что, казак, — сказал он, — поедем?\r\n\r\n***\r\n\r\n«Ёлочки» и ёж.\r\nВторая строка абзаца.\r\n\r\n## Вечер\r\n\r\n„Лапки“ и… многоточие.\r\n\r\n\r\n";
        let c = parse_files(&[file("01.md", messy)], ParagraphMode::BlankLine)
            .unwrap()
            .remove(0);
        let exported = export_chapter(&c.title, &scenes_of(&c));
        assert_eq!(exported, CANONICAL);
        assert_eq!(normalize_markdown(messy), CANONICAL);
        let check = check_file(&file("01.md", messy), &exported, None);
        assert!(check.same_text && check.canonical, "{check:?}");
        for variant in ["---", "___", "* * *", "- - -", "*****"] {
            assert!(
                parse_files(
                    &[file("a.md", &format!("# T\n\nа\n\n{variant}\n\nб\n"))],
                    ParagraphMode::BlankLine
                )
                .unwrap()[0]
                    .scenes
                    .len()
                    == 2,
                "{variant}"
            );
        }
    }

    #[test]
    fn a_missing_chapter_heading_is_taken_from_the_file_name() {
        let chapters = parse_files(
            &[file("03_Начало-пути.md", "Первый абзац.\n\nВторой.\n")],
            ParagraphMode::BlankLine,
        )
        .unwrap();
        let c = &chapters[0];
        assert_eq!(
            (c.title.as_str(), c.title_from_filename),
            ("Начало пути", true)
        );
        let exported = export_chapter(&c.title, &scenes_of(c));
        let check = check_file(
            &file("03_Начало-пути.md", "Первый абзац.\n\nВторой.\n"),
            &exported,
            Some(&c.title),
        );
        assert!(check.same_text && check.canonical, "{check:?}");
        let c2 = &parse_files(&[file("007.md", "Текст.\n")], ParagraphMode::BlankLine).unwrap()[0];
        assert_eq!(c2.title, "Chapter 1");
    }

    #[test]
    fn files_are_read_in_natural_name_order() {
        let chapters = parse_files(
            &[
                file("10.md", "# Десять\n\nа\n"),
                file("2.md", "# Два\n\nб\n"),
            ],
            ParagraphMode::BlankLine,
        )
        .unwrap();
        assert_eq!(
            chapters
                .iter()
                .map(|c| c.title.as_str())
                .collect::<Vec<_>>(),
            ["Два", "Десять"]
        );
    }

    #[test]
    fn stray_breaks_lose_no_text_but_are_not_canonical() {
        let odd = "# Глава\n\n***\n\nАбзац один.\n\n***\n\n***\n\nАбзац два.\n\n***\n";
        let c = parse_files(&[file("a.md", odd)], ParagraphMode::BlankLine)
            .unwrap()
            .remove(0);
        assert_eq!(c.scenes.len(), 2);
        let exported = export_chapter(&c.title, &scenes_of(&c));
        let check = check_file(&file("a.md", odd), &exported, None);
        assert!(check.same_text);
        assert!(
            !check.canonical && check.first_difference_line.is_some(),
            "{check:?}"
        );
    }

    #[test]
    fn paragraph_mode_decides_what_a_single_newline_means() {
        let text = "# Г\n\n— Привет, — сказал он.\n— Привет.\n— Как дела?\n";
        let blank = parse_files(&[file("a.md", text)], ParagraphMode::BlankLine).unwrap();
        assert_eq!(blank[0].scenes[0].paragraphs.len(), 1);
        let line = parse_files(&[file("a.md", text)], ParagraphMode::Line).unwrap();
        assert_eq!(
            line[0].scenes[0].paragraphs,
            ["— Привет, — сказал он.", "— Привет.", "— Как дела?"]
        );
        let exported = export_chapter(&line[0].title, &scenes_of(&line[0]));
        assert!(check_file(&file("a.md", text), &exported, None).same_text);
    }

    #[test]
    fn unsupported_input_is_refused_with_file_and_line_but_never_the_text() {
        let secret = "СЕКРЕТНЫЙ";
        let cases: Vec<(&str, String)> = vec![
            ("###", format!("# Г\n\n### {secret}\n\nтекст\n")),
            ("second #", format!("# Г\n\nтекст\n\n# {secret}\n")),
            ("glued", format!("# Г\n\n## {secret}\nтекст сразу\n")),
            ("empty heading", "# Г\n\n## \n\nтекст\n".to_string()),
            ("hash text", format!("# Г\n\n#{secret}\n")),
            ("marker", format!("# Г\n\n{secret} <!-- p:k7m2x9qa -->\n")),
            ("break in text", format!("# Г\n\n{secret}\n---\nещё\n")),
        ];
        for (name, content) in cases {
            let found = issues(&[file("a.md", &content)]);
            assert!(!found.is_empty(), "{name}");
            for i in &found {
                assert_eq!(i.file, "a.md");
                assert!(!i.message.contains(secret), "{name}: {}", i.message);
            }
        }
        let found = issues(&[file("a.md", "# Г\n\nтекст\n\n### x\n")]);
        assert_eq!((found[0].file.as_str(), found[0].line), ("a.md", 5));
    }

    #[test]
    fn bad_file_lists_are_refused() {
        assert!(!issues(&[]).is_empty());
        for bad in ["notes.txt", "sub/dir.md", "..\\x.md", ".hidden.md", ""] {
            assert!(!issues(&[file(bad, "# Г\n\nтекст\n")]).is_empty(), "{bad}");
        }
        let dup = issues(&[file("a.md", "# А\n"), file("a.md", "# Б\n")]);
        assert!(dup.iter().any(|i| i.message.contains("duplicate")));
    }

    #[test]
    fn many_problems_are_capped() {
        let content: String = (0..200).map(|i| format!("### h{i}\n\n")).collect();
        assert_eq!(
            issues(&[file("a.md", &format!("# Г\n\n{content}"))]).len(),
            MAX_ISSUES
        );
    }

    #[test]
    fn the_text_check_notices_any_loss_or_change() {
        let original = file("a.md", CANONICAL);
        let c = parse_files(std::slice::from_ref(&original), ParagraphMode::BlankLine)
            .unwrap()
            .remove(0);
        let exported = export_chapter(&c.title, &scenes_of(&c));
        assert!(check_file(&original, &exported, None).same_text);
        let broken = [
            exported.replace("холодное, ", ""),       // a word lost
            exported.replace("ёж", "еж"),             // a letter changed
            exported.replace("— Ну что", "- Ну что"), // a dash changed
            exported.replace("и… многоточие", "и... многоточие"), // an ellipsis changed
            format!("{exported}\nлишний абзац\n"),    // text added
        ];
        for b in broken {
            assert!(!check_file(&original, &b, None).same_text, "{b}");
        }
        // two paragraphs swapped
        let swapped = exported
            .replace("Над Доном вставало солнце — холодное, чужое.", "@@")
            .replace(
                "— Ну что, казак, — сказал он, — поедем?",
                "Над Доном вставало солнце — холодное, чужое.",
            )
            .replace("@@", "— Ну что, казак, — сказал он, — поедем?");
        assert!(!check_file(&original, &swapped, None).same_text);
    }

    #[test]
    fn unicode_is_not_normalised() {
        let decomposed = "и\u{306} ё\u{308} \u{a0}неразрывный";
        let c = parse_files(
            &[file("a.md", &format!("# Г\n\n{decomposed}\n"))],
            ParagraphMode::BlankLine,
        )
        .unwrap()
        .remove(0);
        assert_eq!(c.scenes[0].paragraphs[0], decomposed);
    }
}
