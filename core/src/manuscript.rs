//! The scene file format and the rules for passages. Pure functions: no files, no database.
//!
//! A scene file is a TOML front-matter block between `+++` lines, then paragraphs separated by blank
//! lines. Each paragraph (a passage) ends with a hidden marker `<!-- p:ID -->`, which keeps its identity
//! through edits and reordering. Text is never changed beyond the normalisation in `normalize_paragraph`.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::ids;

const MARKER_OPEN: &str = "<!-- p:";
const MARKER_CLOSE: &str = " -->";
const FENCE: &str = "+++";

pub const MAX_FIELD_CHARS: usize = 300;

/// Front-matter of a scene file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneMeta {
    pub id: String,
    #[serde(default)]
    pub title: String,
    pub language: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pov: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
}

/// One paragraph. `id` is `None` for a paragraph that has no marker yet (text edited outside Pero).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Passage {
    pub id: Option<String>,
    pub text: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ParsedScene {
    pub meta: SceneMeta,
    pub passages: Vec<Passage>,
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Hash of a normalised passage text; evidence links remember it to notice later edits.
pub fn text_hash(text: &str) -> String {
    sha256_hex(text.as_bytes())
}

pub fn valid_language(language: &str) -> bool {
    matches!(language, "ru" | "en")
}

/// Checks a single-line metadata value (title, POV, date, location).
pub fn clean_field(label: &str, value: &str) -> Result<String, String> {
    let v = value.trim();
    if v.chars().count() > MAX_FIELD_CHARS {
        return Err(format!(
            "{label} is longer than {MAX_FIELD_CHARS} characters"
        ));
    }
    if v.chars().any(char::is_control) {
        return Err(format!(
            "{label} must be a single line without control characters"
        ));
    }
    Ok(v.to_string())
}

fn is_scene_break(line: &str) -> bool {
    let compact: String = line.chars().filter(|c| !c.is_whitespace()).collect();
    compact.len() >= 3
        && ["*", "-", "_"]
            .iter()
            .any(|ch| compact.chars().all(|c| ch.starts_with(c)))
}

/// Normalises one paragraph: LF line endings, no trailing spaces, no surrounding blank lines.
/// Rejects what could not survive a trip through the Markdown file as a single passage.
pub fn normalize_paragraph(text: &str) -> Result<String, String> {
    let unified = text.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = unified
        .trim_matches('\n')
        .split('\n')
        .map(str::trim_end)
        .collect();
    if lines.iter().all(|l| l.trim().is_empty()) {
        return Err("a passage cannot be empty".into());
    }
    if lines.iter().any(|l| l.trim().is_empty()) {
        return Err("a passage is one paragraph: it cannot contain a blank line".into());
    }
    let out = lines.join("\n");
    if out.contains(MARKER_OPEN) {
        return Err("passage text cannot contain a passage marker".into());
    }
    if out.starts_with('#') || is_scene_break(lines[0]) {
        return Err("a passage cannot start with '#' or be a scene break line".into());
    }
    Ok(out)
}

/// Reads a scene file. Line endings are unified; unmarked paragraphs get `id: None`; a marker that
/// repeats an earlier one (a pasted copy) is treated as unmarked.
pub fn parse_scene(raw: &str) -> Result<ParsedScene, String> {
    let text = raw
        .strip_prefix('\u{feff}')
        .unwrap_or(raw)
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    let rest = text
        .strip_prefix(FENCE)
        .and_then(|r| r.strip_prefix('\n'))
        .ok_or("the file does not start with a +++ front-matter block")?;
    let (front, body) = match rest.split_once(&format!("\n{FENCE}\n")) {
        Some(parts) => parts,
        None => rest
            .strip_suffix(&format!("\n{FENCE}"))
            .map(|f| (f, ""))
            .ok_or("the front-matter block is not closed with +++")?,
    };
    let meta: SceneMeta =
        toml::from_str(front).map_err(|e| format!("invalid front-matter: {e}"))?;

    let mut passages: Vec<Passage> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut block: Vec<&str> = Vec::new();
    let mut flush = |block: &mut Vec<&str>, passages: &mut Vec<Passage>| {
        if block.is_empty() {
            return;
        }
        let mut lines: Vec<String> = block.drain(..).map(|l| l.trim_end().to_string()).collect();
        let last = lines.last_mut().expect("non-empty block");
        let mut id = None;
        if let Some(stripped) = last.strip_suffix(MARKER_CLOSE)
            && let Some(at) = stripped.rfind(MARKER_OPEN)
            && ids::is_code(&stripped[at + MARKER_OPEN.len()..])
        {
            id = Some(stripped[at + MARKER_OPEN.len()..].to_string());
            *last = stripped[..at].trim_end().to_string();
        }
        if last.is_empty() {
            lines.pop();
        }
        if lines.is_empty() {
            return; // a marker with no text: the passage is gone
        }
        let id = id.filter(|i| seen.insert(i.clone()));
        passages.push(Passage {
            id,
            text: lines.join("\n"),
        });
    };
    for line in body.split('\n') {
        if line.trim().is_empty() {
            flush(&mut block, &mut passages);
        } else {
            block.push(line);
        }
    }
    flush(&mut block, &mut passages);
    Ok(ParsedScene { meta, passages })
}

/// Writes a scene file. Every passage must already be normalised and have an ID.
pub fn render_scene(meta: &SceneMeta, passages: &[(String, String)]) -> Result<String, String> {
    let front = toml::to_string(meta).map_err(|e| format!("cannot write front-matter: {e}"))?;
    let mut out = format!("{FENCE}\n{front}{FENCE}\n");
    for (id, text) in passages {
        out.push('\n');
        out.push_str(text);
        out.push(' ');
        out.push_str(MARKER_OPEN);
        out.push_str(id);
        out.push_str(MARKER_CLOSE);
        out.push('\n');
    }
    Ok(out)
}

/// ASCII slug for file and folder names; Cyrillic is transliterated.
pub fn slugify(title: &str, fallback: &str) -> String {
    let mut out = String::new();
    for c in title.to_lowercase().chars() {
        let t = match c {
            'а' => "a",
            'б' => "b",
            'в' => "v",
            'г' => "g",
            'д' => "d",
            'е' | 'ё' | 'э' => "e",
            'ж' => "zh",
            'з' => "z",
            'и' => "i",
            'й' | 'ы' => "y",
            'к' => "k",
            'л' => "l",
            'м' => "m",
            'н' => "n",
            'о' => "o",
            'п' => "p",
            'р' => "r",
            'с' => "s",
            'т' => "t",
            'у' => "u",
            'ф' => "f",
            'х' => "kh",
            'ц' => "ts",
            'ч' => "ch",
            'ш' => "sh",
            'щ' => "shch",
            'ъ' | 'ь' => "",
            'ю' => "yu",
            'я' => "ya",
            c if c.is_ascii_alphanumeric() => {
                out.push(c);
                continue;
            }
            _ => "-",
        };
        out.push_str(t);
    }
    let slug: String = out
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let slug: String = slug.chars().take(40).collect();
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        fallback.to_string()
    } else {
        slug.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta() -> SceneMeta {
        SceneMeta {
            id: "sc_k7m2x9qa".into(),
            title: "Утро «на хуторе»".into(),
            language: "ru".into(),
            pov: Some("Григорий".into()),
            date: None,
            location: Some("хутор Татарский".into()),
        }
    }

    const P1: &str = "Над Доном вставало солнце — холодное, чужое.";
    const P2: &str = "— Ну что, казак, — сказал он, — поедем?";
    const P3: &str = "«Ёлочки» и ёж.\nВторая строка абзаца.";

    #[test]
    fn rendering_then_parsing_gives_back_the_same_scene() {
        let pairs = vec![
            ("k7m2x9qa".to_string(), P1.to_string()),
            ("p3n8d5wz".to_string(), P2.to_string()),
            ("q1r2s3t4".to_string(), P3.to_string()),
        ];
        let file = render_scene(&meta(), &pairs).unwrap();
        assert!(file.contains("Ёлочки» и ёж.\nВторая строка абзаца. <!-- p:q1r2s3t4 -->\n"));
        let parsed = parse_scene(&file).unwrap();
        assert_eq!(parsed.meta, meta());
        let got: Vec<(String, String)> = parsed
            .passages
            .into_iter()
            .map(|p| (p.id.unwrap(), p.text))
            .collect();
        assert_eq!(got, pairs);
    }

    #[test]
    fn an_empty_scene_round_trips() {
        let file = render_scene(&meta(), &[]).unwrap();
        assert!(file.ends_with("+++\n"));
        assert!(parse_scene(&file).unwrap().passages.is_empty());
    }

    #[test]
    fn reordering_paragraphs_in_the_file_keeps_each_id_with_its_text() {
        let file = render_scene(
            &meta(),
            &[
                ("k7m2x9qa".into(), P1.into()),
                ("p3n8d5wz".into(), P2.into()),
            ],
        )
        .unwrap();
        let swapped = parse_scene(&file).unwrap();
        let mut ps = swapped.passages;
        ps.reverse();
        assert_eq!(ps[0].id.as_deref(), Some("p3n8d5wz"));
        assert_eq!(ps[0].text, P2);
    }

    #[test]
    fn unmarked_and_repeated_markers_become_unmarked() {
        let file = "+++\nid = \"sc_k7m2x9qa\"\nlanguage = \"ru\"\n+++\n\nОдин <!-- p:k7m2x9qa -->\n\nДва\n\nТри <!-- p:k7m2x9qa -->\n";
        let ps = parse_scene(file).unwrap().passages;
        assert_eq!(ps.len(), 3);
        assert_eq!(ps[0].id.as_deref(), Some("k7m2x9qa"));
        assert_eq!(
            ps[1],
            Passage {
                id: None,
                text: "Два".into()
            }
        );
        assert_eq!(
            ps[2],
            Passage {
                id: None,
                text: "Три".into()
            }
        );
    }

    #[test]
    fn windows_line_endings_and_a_bom_are_accepted() {
        let file = "\u{feff}+++\r\nid = \"sc_k7m2x9qa\"\r\nlanguage = \"en\"\r\n+++\r\n\r\nHello <!-- p:k7m2x9qa -->\r\n";
        let parsed = parse_scene(file).unwrap();
        assert_eq!(parsed.passages[0].text, "Hello");
        assert_eq!(parsed.meta.language, "en");
    }

    #[test]
    fn broken_files_are_reported_without_quoting_the_prose() {
        assert!(parse_scene("no front matter\n\ntext").is_err());
        assert!(parse_scene("+++\nid = \"x\"\nlanguage = \"ru\"\n").is_err());
        let err =
            parse_scene("+++\nid = \"sc_k7m2x9qa\"\nlanguage = \"ru\"\nmood = \"sad\"\n+++\n")
                .unwrap_err();
        assert!(err.contains("mood"), "{err}");
    }

    #[test]
    fn normalising_a_paragraph_fixes_only_whitespace() {
        assert_eq!(
            normalize_paragraph("  Привет, мир.  \r\n  вторая  \n\n").unwrap(),
            "  Привет, мир.\n  вторая"
        );
        assert_eq!(normalize_paragraph(P3).unwrap(), P3);
        assert_eq!(normalize_paragraph("— Да.").unwrap(), "— Да.");
        // no Unicode normalisation: a decomposed 'й' stays decomposed
        let decomposed = "и\u{306}";
        assert_eq!(normalize_paragraph(decomposed).unwrap(), decomposed);
    }

    #[test]
    fn paragraphs_that_cannot_round_trip_are_rejected() {
        for bad in [
            "",
            "   \n ",
            "a\n\nb",
            "# Заголовок",
            "***",
            "* * *",
            "---",
            "text <!-- p:k7m2x9qa --> more",
        ] {
            assert!(normalize_paragraph(bad).is_err(), "{bad:?}");
        }
        assert!(normalize_paragraph("-- не разделитель").is_ok());
    }

    #[test]
    fn slugs_are_ascii_and_stable() {
        assert_eq!(
            slugify("Глава первая: «Утро»", "chapter"),
            "glava-pervaya-utro"
        );
        assert_eq!(slugify("Щёлково, ёжик", "x"), "shchelkovo-ezhik");
        assert_eq!(slugify("???", "scene"), "scene");
        assert_eq!(slugify("", "scene"), "scene");
        assert!(slugify(&"а".repeat(100), "x").len() <= 40);
    }

    #[test]
    fn field_cleaning() {
        assert_eq!(clean_field("title", "  Утро ").unwrap(), "Утро");
        assert!(clean_field("title", "a\nb").is_err());
        assert!(clean_field("title", &"я".repeat(301)).is_err());
    }
}
