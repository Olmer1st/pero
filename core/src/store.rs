//! Manuscript store: one Git repository per book (Markdown files) plus an index in SQLite.
//!
//! The files are the only copy of the prose. Every change to a book is one Git commit made under a
//! per-book lock: the working tree must be clean, the change is written, committed, and the index is
//! rebuilt from the repository. If anything fails before the commit the change is rolled back.
//!
//! Layout of a book repository (`<root>/<book-id>/`):
//!   book.toml                          id, title, language
//!   chapters/NNN-slug/chapter.toml     id, title
//!   chapters/NNN-slug/NNN-slug.md      one scene: TOML front-matter and paragraphs with ID markers
//! Order is the numeric prefix; a reorder is a rename. IDs live inside the files and never change.

use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
};

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tokio::sync::{Mutex, OwnedMutexGuard};

use crate::{
    git, ids,
    import::{self, ImportFile, Issue, ParagraphMode},
    manuscript::{self, Passage, SceneMeta},
};

const MAX_ITEMS: usize = 999;

#[derive(Debug)]
pub enum StoreError {
    NotFound(String),
    Invalid(String),
    /// The scene changed since the client read it; carries the current version.
    Conflict(Box<Scene>),
    /// The repository has uncommitted changes, so no further change is made until the owner looks.
    Dirty,
    /// The files to import were not understood; nothing was imported.
    ImportRejected(Vec<Issue>),
    /// The same thing already exists.
    Exists(String),
    /// Something else still depends on it, so it cannot be deleted.
    InUse(String),
    /// The requested status change is not allowed from the current status.
    Transition(String),
    /// No valid device token.
    Unauthorized,
    Internal(String),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::NotFound(m)
            | StoreError::Invalid(m)
            | StoreError::Exists(m)
            | StoreError::InUse(m)
            | StoreError::Transition(m)
            | StoreError::Internal(m) => f.write_str(m),
            StoreError::Conflict(_) => f.write_str("the scene changed since it was read"),
            StoreError::Dirty => f.write_str("the book repository has uncommitted changes"),
            StoreError::Unauthorized => f.write_str("a valid device token is required"),
            StoreError::ImportRejected(issues) => {
                write!(f, "the import was refused ({} problems)", issues.len())
            }
        }
    }
}

impl From<sqlx::Error> for StoreError {
    fn from(e: sqlx::Error) -> Self {
        StoreError::Internal(format!("database error: {e}"))
    }
}

fn internal(e: impl std::fmt::Display) -> StoreError {
    StoreError::Internal(e.to_string())
}

fn invalid(e: impl Into<String>) -> StoreError {
    StoreError::Invalid(e.into())
}

// ---- data shown to clients -------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Book {
    pub id: String,
    pub title: String,
    pub language: String,
    pub head_commit: Option<String>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Chapter {
    pub id: String,
    pub book_id: String,
    pub position: i64,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct SceneInfo {
    pub id: String,
    pub book_id: String,
    pub chapter_id: String,
    pub position: i64,
    pub title: String,
    pub language: String,
    pub pov: Option<String>,
    pub date: Option<String>,
    pub location: Option<String>,
}

#[derive(sqlx::FromRow)]
struct SceneRow {
    #[sqlx(flatten)]
    info: SceneInfo,
    path: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PassageOut {
    /// `None` for a paragraph written outside Pero that has no marker yet.
    pub id: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Scene {
    #[serde(flatten)]
    pub info: SceneInfo,
    pub passages: Vec<PassageOut>,
    /// SHA-256 of the scene file; send it back as `base_revision` when saving.
    pub revision: String,
}

/// Scene metadata as the client sends it. Saving replaces all of it.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneFields {
    #[serde(default)]
    pub title: String,
    pub language: Option<String>,
    pub pov: Option<String>,
    pub date: Option<String>,
    pub location: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PassageIn {
    /// Existing passage ID, or absent for a new paragraph.
    pub id: Option<String>,
    pub text: String,
}

/// A passage that was merged into another one (its identity moves there for evidence purposes).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Merge {
    pub from: String,
    pub into: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveScene {
    pub base_revision: String,
    pub fields: SceneFields,
    pub passages: Vec<PassageIn>,
    #[serde(default)]
    pub merged: Vec<Merge>,
}

#[derive(Debug, Serialize)]
pub struct Saved {
    pub scene: Scene,
    /// The new commit, or `None` when the save changed nothing.
    pub commit: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ImportSummary {
    pub book: Book,
    pub chapters: usize,
    pub scenes: usize,
    pub passages: usize,
    /// Files that had no `# ` heading, so the chapter title was made from the file name.
    pub titles_from_filename: Vec<String>,
}

/// A chapter written back out as Markdown.
#[derive(Debug, Serialize)]
pub struct ExportFile {
    pub name: String,
    pub content: String,
    /// True when the chapter title is the one the import made up from a file name.
    #[serde(skip)]
    pub title: String,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct SyncReport {
    /// Paragraphs found without a marker; they have no stable ID until a save assigns one.
    pub unmarked: usize,
}

// ---- files in the repository -----------------------------------------------------------------

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BookFile {
    id: String,
    title: String,
    language: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChapterFile {
    id: String,
    title: String,
}

struct SceneSnap {
    meta: SceneMeta,
    file: String,
    passages: Vec<Passage>,
}

struct ChapterSnap {
    meta: ChapterFile,
    dir: String,
    scenes: Vec<SceneSnap>,
}

fn read_toml<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    let text =
        fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    toml::from_str(&text).map_err(|e| format!("invalid {}: {e}", path.display()))
}

fn write_atomic(path: &Path, content: &str) -> Result<(), String> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("bad file name")?;
    let tmp = path.with_file_name(format!(".{name}.tmp"));
    let mut f =
        fs::File::create(&tmp).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
    f.write_all(content.as_bytes())
        .and_then(|_| f.sync_all())
        .map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| format!("cannot replace {}: {e}", path.display()))
}

/// Names in `dir` (not starting with a dot), sorted: directories or files with the given extension.
fn list_sorted(dir: &Path, dirs: bool, ext: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir(dir) else {
        return Ok(out);
    };
    for entry in rd {
        let entry = entry.map_err(|e| format!("cannot list {}: {e}", dir.display()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_dir = entry.file_type().map_err(|e| e.to_string())?.is_dir();
        if !name.starts_with('.') && is_dir == dirs && (dirs || name.ends_with(ext)) {
            out.push(name);
        }
    }
    out.sort();
    Ok(out)
}

/// `001-slug` or `001-slug.md` to `slug`.
fn slug_of(name: &str) -> String {
    let stem = name.strip_suffix(".md").unwrap_or(name);
    match stem.split_once('-') {
        Some((n, rest)) if n.len() == 3 && n.bytes().all(|b| b.is_ascii_digit()) => {
            rest.to_string()
        }
        _ => stem.to_string(),
    }
}

/// Renames the items (given in the wanted order as `(current name, slug)`) to `NNN-slug<ext>`.
/// Two phases, so names that swap places never collide.
fn renumber(dir: &Path, items: &[(String, String)], ext: &str) -> Result<(), StoreError> {
    if items.len() > MAX_ITEMS {
        return Err(invalid(format!("more than {MAX_ITEMS} items in one place")));
    }
    let targets: Vec<String> = items
        .iter()
        .enumerate()
        .map(|(i, (_, slug))| format!("{:03}-{slug}{ext}", i + 1))
        .collect();
    let moving: Vec<usize> = (0..items.len())
        .filter(|&i| items[i].0 != targets[i])
        .collect();
    for &i in &moving {
        fs::rename(dir.join(&items[i].0), dir.join(format!(".moving-{i}"))).map_err(internal)?;
    }
    for &i in &moving {
        fs::rename(dir.join(format!(".moving-{i}")), dir.join(&targets[i])).map_err(internal)?;
    }
    Ok(())
}

fn read_snapshot(book_dir: &Path) -> Result<(BookFile, Vec<ChapterSnap>), String> {
    let book: BookFile = read_toml(&book_dir.join("book.toml"))?;
    let chapters_dir = book_dir.join("chapters");
    let mut chapters = Vec::new();
    for dir in list_sorted(&chapters_dir, true, "")? {
        let path = chapters_dir.join(&dir);
        let meta: ChapterFile = read_toml(&path.join("chapter.toml"))?;
        let mut scenes = Vec::new();
        for file in list_sorted(&path, false, ".md")? {
            let raw = fs::read_to_string(path.join(&file))
                .map_err(|e| format!("cannot read chapters/{dir}/{file}: {e}"))?;
            let parsed =
                manuscript::parse_scene(&raw).map_err(|e| format!("chapters/{dir}/{file}: {e}"))?;
            scenes.push(SceneSnap {
                meta: parsed.meta,
                file,
                passages: parsed.passages,
            });
        }
        chapters.push(ChapterSnap { meta, dir, scenes });
    }
    Ok((book, chapters))
}

// ---- the store -------------------------------------------------------------------------------

pub struct Store {
    pool: SqlitePool,
    root: PathBuf,
    locks: std::sync::Mutex<HashMap<String, Arc<Mutex<()>>>>,
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> T + Send + 'static,
) -> Result<T, StoreError> {
    tokio::task::spawn_blocking(f).await.map_err(internal)
}

impl Store {
    /// `root` is the folder that holds one sub-folder per book; it is created if missing.
    pub fn new(pool: SqlitePool, root: PathBuf) -> Result<Arc<Store>, String> {
        fs::create_dir_all(&root).map_err(|e| format!("cannot create {}: {e}", root.display()))?;
        Ok(Arc::new(Store {
            pool,
            root,
            locks: Default::default(),
        }))
    }

    /// The SQLite pool, for the story graph and the canon ledger (database only, no Git).
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    fn book_dir(&self, book_id: &str) -> Result<PathBuf, StoreError> {
        if !ids::is_id(book_id, ids::BOOK) {
            return Err(StoreError::NotFound("book not found".into()));
        }
        Ok(self.root.join(book_id))
    }

    async fn lock(&self, book_id: &str) -> OwnedMutexGuard<()> {
        let m = {
            let mut map = self.locks.lock().expect("lock table");
            map.entry(book_id.to_string()).or_default().clone()
        };
        m.lock_owned().await
    }

    /// Writes a change to the repository as one commit (see the module comment). Blocking work
    /// runs off the async threads. Returns the new commit, or `None` if nothing changed.
    async fn commit_change(
        &self,
        book_id: &str,
        actor: &str,
        message: String,
        change: impl FnOnce(&Path) -> Result<(), StoreError> + Send + 'static,
    ) -> Result<Option<String>, StoreError> {
        let dir = self.book_dir(book_id)?;
        let actor = actor.to_string();
        blocking(move || {
            if !git::is_clean(&dir, &actor).map_err(internal)? {
                return Err(StoreError::Dirty);
            }
            if let Err(e) = change(&dir) {
                let _ = git::rollback(&dir, &actor);
                return Err(e);
            }
            git::commit_all(&dir, &actor, &message).map_err(|e| {
                let _ = git::rollback(&dir, &actor);
                internal(e)
            })
        })
        .await?
    }

    // ---- index -------------------------------------------------------------------------------

    /// Rebuilds the index of one book from its repository. `merges` are `(removed, into)` hints from
    /// the save that is being indexed. Passages that vanished from the files become tombstones.
    pub async fn reindex_book(&self, book_id: &str) -> Result<SyncReport, StoreError> {
        let _guard = self.lock(book_id).await;
        self.sync(book_id, &[]).await
    }

    async fn sync(
        &self,
        book_id: &str,
        merges: &[(String, String)],
    ) -> Result<SyncReport, StoreError> {
        let dir = self.book_dir(book_id)?;
        let (book, chapters, head) = blocking(move || {
            let (book, chapters) = read_snapshot(&dir)?;
            let head = git::head(&dir, "pero")?;
            Ok::<_, String>((book, chapters, head))
        })
        .await?
        .map_err(internal)?;
        if book.id != book_id {
            return Err(internal("book.toml does not match its folder"));
        }
        let merges: HashMap<&str, &str> = merges
            .iter()
            .map(|(f, t)| (f.as_str(), t.as_str()))
            .collect();
        let mut report = SyncReport::default();
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            "INSERT INTO books (id, title, language, head_commit) VALUES (?, ?, ?, ?) \
             ON CONFLICT(id) DO UPDATE SET title = excluded.title, language = excluded.language, \
             head_commit = excluded.head_commit, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
        )
        .bind(&book.id)
        .bind(&book.title)
        .bind(&book.language)
        .bind(&head)
        .execute(&mut *tx)
        .await?;

        // Paths are unique per book; free them first so a reorder that swaps names cannot collide.
        sqlx::query("UPDATE scenes SET path = '~' || id WHERE book_id = ?")
            .bind(book_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE chapters SET path = '~' || id WHERE book_id = ?")
            .bind(book_id)
            .execute(&mut *tx)
            .await?;

        let mut active: HashSet<&str> = HashSet::new();
        for (ci, ch) in chapters.iter().enumerate() {
            sqlx::query(
                "INSERT INTO chapters (id, book_id, position, title, path) VALUES (?, ?, ?, ?, ?) \
                 ON CONFLICT(id) DO UPDATE SET position = excluded.position, title = excluded.title, \
                 path = excluded.path, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
                 WHERE chapters.book_id = excluded.book_id",
            )
            .bind(&ch.meta.id)
            .bind(book_id)
            .bind(ci as i64)
            .bind(&ch.meta.title)
            .bind(format!("chapters/{}", ch.dir))
            .execute(&mut *tx)
            .await?;
            for (si, sc) in ch.scenes.iter().enumerate() {
                sqlx::query(
                    "INSERT INTO scenes (id, book_id, chapter_id, position, title, path, language, pov, story_date, location) \
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
                     ON CONFLICT(id) DO UPDATE SET chapter_id = excluded.chapter_id, position = excluded.position, \
                     title = excluded.title, path = excluded.path, language = excluded.language, pov = excluded.pov, \
                     story_date = excluded.story_date, location = excluded.location, \
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE scenes.book_id = excluded.book_id",
                )
                .bind(&sc.meta.id)
                .bind(book_id)
                .bind(&ch.meta.id)
                .bind(si as i64)
                .bind(&sc.meta.title)
                .bind(format!("chapters/{}/{}", ch.dir, sc.file))
                .bind(&sc.meta.language)
                .bind(&sc.meta.pov)
                .bind(&sc.meta.date)
                .bind(&sc.meta.location)
                .execute(&mut *tx)
                .await?;
                for (pi, p) in sc.passages.iter().enumerate() {
                    let Some(id) = &p.id else {
                        report.unmarked += 1;
                        continue;
                    };
                    let res = sqlx::query(
                        "INSERT INTO passages (id, book_id, scene_id, position, text_hash, status) \
                         VALUES (?, ?, ?, ?, ?, 'active') \
                         ON CONFLICT(id) DO UPDATE SET scene_id = excluded.scene_id, position = excluded.position, \
                         updated_at = CASE WHEN passages.text_hash <> excluded.text_hash OR passages.status <> 'active' \
                             THEN strftime('%Y-%m-%dT%H:%M:%fZ', 'now') ELSE passages.updated_at END, \
                         text_hash = excluded.text_hash, status = 'active', removed_at = NULL, merged_into = NULL \
                         WHERE passages.book_id = excluded.book_id",
                    )
                    .bind(id)
                    .bind(book_id)
                    .bind(&sc.meta.id)
                    .bind(pi as i64)
                    .bind(manuscript::text_hash(&p.text))
                    .execute(&mut *tx)
                    .await?;
                    if res.rows_affected() == 0 {
                        return Err(internal("a passage ID belongs to another book"));
                    }
                    active.insert(id);
                }
            }
        }

        let vanished: Vec<(String,)> =
            sqlx::query_as("SELECT id FROM passages WHERE book_id = ? AND status = 'active'")
                .bind(book_id)
                .fetch_all(&mut *tx)
                .await?;
        for (id,) in vanished {
            if active.contains(id.as_str()) {
                continue;
            }
            let into = merges
                .get(id.as_str())
                .filter(|t| active.contains(*t))
                .map(|t| t.to_string());
            sqlx::query(
                "UPDATE passages SET status = 'removed', removed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), \
                 merged_into = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
            )
            .bind(into)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        }
        // Facts that cite a passage removed, merged or edited by this change are flagged, never deleted.
        crate::canon::flag_stale(&mut tx, book_id).await?;
        tx.commit().await?;
        Ok(report)
    }

    // ---- books -------------------------------------------------------------------------------

    pub async fn create_book(
        &self,
        title: &str,
        language: &str,
        actor: &str,
    ) -> Result<Book, StoreError> {
        let title = non_empty("title", title)?;
        if !manuscript::valid_language(language) {
            return Err(invalid("language must be \"ru\" or \"en\""));
        }
        let id = ids::new_id(ids::BOOK);
        let dir = self.book_dir(&id)?;
        let _guard = self.lock(&id).await;
        let file = BookFile {
            id: id.clone(),
            title: title.clone(),
            language: language.to_string(),
        };
        let (d, a) = (dir.clone(), actor.to_string());
        let created = blocking(move || -> Result<(), String> {
            fs::create_dir_all(d.join("chapters")).map_err(|e| e.to_string())?;
            write_atomic(
                &d.join("book.toml"),
                &toml::to_string(&file).map_err(|e| e.to_string())?,
            )?;
            git::init(&d, &a)?;
            git::commit_all(&d, &a, &format!("Create book: {}", file.title))?;
            Ok(())
        })
        .await?;
        if let Err(e) = created {
            let _ = fs::remove_dir_all(&dir);
            return Err(internal(e));
        }
        self.sync(&id, &[]).await?;
        self.get_book(&id).await
    }

    pub async fn get_book(&self, id: &str) -> Result<Book, StoreError> {
        sqlx::query_as("SELECT id, title, language, head_commit FROM books WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| StoreError::NotFound("book not found".into()))
    }

    pub async fn list_books(&self) -> Result<Vec<Book>, StoreError> {
        Ok(sqlx::query_as(
            "SELECT id, title, language, head_commit FROM books ORDER BY created_at, id",
        )
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn update_book_title(
        &self,
        id: &str,
        title: &str,
        actor: &str,
    ) -> Result<Book, StoreError> {
        let title = non_empty("title", title)?;
        let current = self.get_book(id).await?;
        let _guard = self.lock(id).await;
        let file = BookFile {
            id: current.id.clone(),
            title: title.clone(),
            language: current.language,
        };
        self.commit_change(id, actor, format!("Rename book: {title}"), move |dir| {
            write_atomic(
                &dir.join("book.toml"),
                &toml::to_string(&file).map_err(internal)?,
            )
            .map_err(internal)
        })
        .await?;
        self.sync(id, &[]).await?;
        self.get_book(id).await
    }

    // ---- import and export -------------------------------------------------------------------

    /// Creates a book from a set of Markdown chapter files (one file per chapter) as ONE commit.
    /// Either everything is understood and imported, or nothing is (`ImportRejected` lists why).
    pub async fn import_book(
        &self,
        title: &str,
        language: &str,
        mode: ParagraphMode,
        files: &[ImportFile],
        actor: &str,
    ) -> Result<ImportSummary, StoreError> {
        let title = non_empty("title", title)?;
        if !manuscript::valid_language(language) {
            return Err(invalid("language must be \"ru\" or \"en\""));
        }
        let parsed = import::parse_files(files, mode).map_err(StoreError::ImportRejected)?;

        let book_id = ids::new_id(ids::BOOK);
        let dir = self.book_dir(&book_id)?;
        let _guard = self.lock(&book_id).await;

        // Plan every file first (IDs included), so the blocking part only writes.
        struct PlannedScene {
            file: String,
            content: String,
        }
        struct PlannedChapter {
            dir: String,
            toml: String,
            scenes: Vec<PlannedScene>,
        }
        let mut used: HashSet<String> = HashSet::new();
        let (mut scene_count, mut passage_count) = (0, 0);
        let mut plan: Vec<PlannedChapter> = Vec::new();
        for (ci, ch) in parsed.iter().enumerate() {
            let chapter_file = ChapterFile {
                id: ids::new_id(ids::CHAPTER),
                title: ch.title.clone(),
            };
            let mut scenes = Vec::new();
            for (si, sc) in ch.scenes.iter().enumerate() {
                let meta = SceneMeta {
                    id: ids::new_id(ids::SCENE),
                    title: sc.title.clone(),
                    language: language.to_string(),
                    pov: None,
                    date: None,
                    location: None,
                };
                let mut pairs = Vec::with_capacity(sc.paragraphs.len());
                for text in &sc.paragraphs {
                    pairs.push((self.fresh_passage_id(&mut used).await?, text.clone()));
                }
                passage_count += pairs.len();
                scene_count += 1;
                scenes.push(PlannedScene {
                    file: format!(
                        "{:03}-{}.md",
                        si + 1,
                        manuscript::slugify(&sc.title, "scene")
                    ),
                    content: manuscript::render_scene(&meta, &pairs).map_err(internal)?,
                });
            }
            plan.push(PlannedChapter {
                dir: format!(
                    "{:03}-{}",
                    ci + 1,
                    manuscript::slugify(&ch.title, "chapter")
                ),
                toml: toml::to_string(&chapter_file).map_err(internal)?,
                scenes,
            });
        }

        let book_file = BookFile {
            id: book_id.clone(),
            title: title.clone(),
            language: language.to_string(),
        };
        let message = format!("Import book: {title} ({} chapters)", plan.len());
        let (d, a) = (dir.clone(), actor.to_string());
        let written = blocking(move || -> Result<(), String> {
            fs::create_dir_all(d.join("chapters")).map_err(|e| e.to_string())?;
            write_atomic(
                &d.join("book.toml"),
                &toml::to_string(&book_file).map_err(|e| e.to_string())?,
            )?;
            for ch in &plan {
                let cdir = d.join("chapters").join(&ch.dir);
                fs::create_dir_all(&cdir).map_err(|e| e.to_string())?;
                write_atomic(&cdir.join("chapter.toml"), &ch.toml)?;
                for sc in &ch.scenes {
                    write_atomic(&cdir.join(&sc.file), &sc.content)?;
                }
            }
            git::init(&d, &a)?;
            git::commit_all(&d, &a, &message)?;
            Ok(())
        })
        .await?;
        if let Err(e) = written {
            let _ = fs::remove_dir_all(&dir);
            return Err(internal(e));
        }
        if let Err(e) = self.sync(&book_id, &[]).await {
            let _ = fs::remove_dir_all(&dir);
            return Err(e);
        }
        Ok(ImportSummary {
            book: self.get_book(&book_id).await?,
            chapters: parsed.len(),
            scenes: scene_count,
            passages: passage_count,
            titles_from_filename: parsed
                .iter()
                .filter(|c| c.title_from_filename)
                .map(|c| c.file.clone())
                .collect(),
        })
    }

    async fn fresh_passage_id(&self, used: &mut HashSet<String>) -> Result<String, StoreError> {
        loop {
            let candidate = ids::code();
            let taken: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM passages WHERE id = ?")
                .bind(&candidate)
                .fetch_optional(&self.pool)
                .await?;
            if taken.is_none() && used.insert(candidate.clone()) {
                return Ok(candidate);
            }
        }
    }

    /// Every chapter of the book as canonical Markdown (headings, scene breaks, plain paragraphs).
    pub async fn export_book(&self, book_id: &str) -> Result<Vec<ExportFile>, StoreError> {
        self.get_book(book_id).await?;
        let dir = self.book_dir(book_id)?;
        let _guard = self.lock(book_id).await;
        let (_, chapters) = blocking(move || read_snapshot(&dir))
            .await?
            .map_err(internal)?;
        Ok(chapters
            .iter()
            .map(|ch| {
                let scenes: Vec<(String, Vec<String>)> = ch
                    .scenes
                    .iter()
                    .map(|sc| {
                        (
                            sc.meta.title.clone(),
                            sc.passages.iter().map(|p| p.text.clone()).collect(),
                        )
                    })
                    .collect();
                ExportFile {
                    name: format!("{}.md", ch.dir),
                    content: import::export_chapter(&ch.meta.title, &scenes),
                    title: ch.meta.title.clone(),
                }
            })
            .collect())
    }

    // ---- chapters ----------------------------------------------------------------------------

    pub async fn get_chapter(&self, id: &str) -> Result<Chapter, StoreError> {
        sqlx::query_as("SELECT id, book_id, position, title FROM chapters WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| StoreError::NotFound("chapter not found".into()))
    }

    pub async fn list_chapters(&self, book_id: &str) -> Result<Vec<Chapter>, StoreError> {
        self.get_book(book_id).await?;
        Ok(sqlx::query_as(
            "SELECT id, book_id, position, title FROM chapters WHERE book_id = ? ORDER BY position",
        )
        .bind(book_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn create_chapter(
        &self,
        book_id: &str,
        title: &str,
        position: Option<usize>,
        actor: &str,
    ) -> Result<Chapter, StoreError> {
        let title = non_empty("title", title)?;
        self.get_book(book_id).await?;
        let _guard = self.lock(book_id).await;
        let id = ids::new_id(ids::CHAPTER);
        let (cid, ctitle) = (id.clone(), title.clone());
        self.commit_change(
            book_id,
            actor,
            format!("Add chapter: {title}"),
            move |dir| {
                let chapters = dir.join("chapters");
                let mut items: Vec<(String, String)> = list_sorted(&chapters, true, "")
                    .map_err(internal)?
                    .into_iter()
                    .map(|n| (n.clone(), slug_of(&n)))
                    .collect();
                let temp = format!(".new-{cid}");
                fs::create_dir_all(chapters.join(&temp)).map_err(internal)?;
                let file = ChapterFile {
                    id: cid.clone(),
                    title: ctitle.clone(),
                };
                write_atomic(
                    &chapters.join(&temp).join("chapter.toml"),
                    &toml::to_string(&file).map_err(internal)?,
                )
                .map_err(internal)?;
                let at = position.unwrap_or(items.len()).min(items.len());
                items.insert(at, (temp, manuscript::slugify(&ctitle, "chapter")));
                renumber(&chapters, &items, "")
            },
        )
        .await?;
        self.sync(book_id, &[]).await?;
        self.get_chapter(&id).await
    }

    pub async fn update_chapter_title(
        &self,
        id: &str,
        title: &str,
        actor: &str,
    ) -> Result<Chapter, StoreError> {
        let title = non_empty("title", title)?;
        let chapter = self.get_chapter(id).await?;
        let _guard = self.lock(&chapter.book_id).await;
        let path = self.chapter_path(id).await?;
        let file = ChapterFile {
            id: id.to_string(),
            title: title.clone(),
        };
        self.commit_change(
            &chapter.book_id,
            actor,
            format!("Rename chapter: {title}"),
            move |dir| {
                write_atomic(
                    &dir.join(path).join("chapter.toml"),
                    &toml::to_string(&file).map_err(internal)?,
                )
                .map_err(internal)
            },
        )
        .await?;
        self.sync(&chapter.book_id, &[]).await?;
        self.get_chapter(id).await
    }

    /// `ids` must name every chapter of the book exactly once, in the wanted order.
    pub async fn reorder_chapters(
        &self,
        book_id: &str,
        ids: &[String],
        actor: &str,
    ) -> Result<Vec<Chapter>, StoreError> {
        let _guard = self.lock(book_id).await;
        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT id, path FROM chapters WHERE book_id = ?")
                .bind(book_id)
                .fetch_all(&self.pool)
                .await?;
        let items = ordered_items(&rows, ids, "chapter")?;
        self.commit_change(book_id, actor, "Reorder chapters".to_string(), move |dir| {
            renumber(&dir.join("chapters"), &items, "")
        })
        .await?;
        self.sync(book_id, &[]).await?;
        self.list_chapters(book_id).await
    }

    async fn chapter_path(&self, chapter_id: &str) -> Result<String, StoreError> {
        let row: Option<(String,)> = sqlx::query_as("SELECT path FROM chapters WHERE id = ?")
            .bind(chapter_id)
            .fetch_optional(&self.pool)
            .await?;
        row.map(|r| r.0)
            .ok_or_else(|| StoreError::NotFound("chapter not found".into()))
    }

    // ---- scenes ------------------------------------------------------------------------------

    pub async fn list_scenes(&self, chapter_id: &str) -> Result<Vec<SceneInfo>, StoreError> {
        self.get_chapter(chapter_id).await?;
        Ok(sqlx::query_as(SCENE_SELECT_BY_CHAPTER)
            .bind(chapter_id)
            .fetch_all(&self.pool)
            .await?)
    }

    async fn scene_row(&self, scene_id: &str) -> Result<(SceneInfo, String), StoreError> {
        if !ids::is_id(scene_id, ids::SCENE) {
            return Err(StoreError::NotFound("scene not found".into()));
        }
        let row: SceneRow = sqlx::query_as(
            "SELECT id, book_id, chapter_id, position, title, language, pov, story_date AS date, location, path \
             FROM scenes WHERE id = ?",
        )
        .bind(scene_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| StoreError::NotFound("scene not found".into()))?;
        Ok((row.info, row.path))
    }

    /// Reads and parses the scene file; also returns the file's text as it is on disk.
    async fn load_scene(
        &self,
        info: SceneInfo,
        path: String,
    ) -> Result<(Scene, String), StoreError> {
        let file = self.book_dir(&info.book_id)?.join(path);
        let raw = blocking(move || {
            fs::read(&file).map_err(|e| format!("cannot read the scene file: {e}"))
        })
        .await?
        .map_err(internal)?;
        let text = String::from_utf8(raw.clone())
            .map_err(|_| internal("the scene file is not valid UTF-8"))?;
        let parsed = manuscript::parse_scene(&text).map_err(internal)?;
        let info = SceneInfo {
            title: parsed.meta.title,
            language: parsed.meta.language,
            pov: parsed.meta.pov,
            date: parsed.meta.date,
            location: parsed.meta.location,
            ..info
        };
        let scene = Scene {
            info,
            passages: parsed
                .passages
                .into_iter()
                .map(|p| PassageOut {
                    id: p.id,
                    text: p.text,
                })
                .collect(),
            revision: manuscript::sha256_hex(&raw),
        };
        Ok((scene, text))
    }

    pub async fn read_scene(&self, scene_id: &str) -> Result<Scene, StoreError> {
        let (info, path) = self.scene_row(scene_id).await?;
        Ok(self.load_scene(info, path).await?.0)
    }

    pub async fn create_scene(
        &self,
        chapter_id: &str,
        fields: &SceneFields,
        position: Option<usize>,
        actor: &str,
    ) -> Result<Scene, StoreError> {
        let chapter = self.get_chapter(chapter_id).await?;
        let book = self.get_book(&chapter.book_id).await?;
        let id = ids::new_id(ids::SCENE);
        let meta = build_meta(&id, fields, &book.language)?;
        let content = manuscript::render_scene(&meta, &[]).map_err(internal)?;
        let _guard = self.lock(&chapter.book_id).await;
        let chapter_path = self.chapter_path(chapter_id).await?;
        let slug = manuscript::slugify(&meta.title, "scene");
        let message = format!(
            "Add scene: {}",
            if meta.title.is_empty() {
                &id
            } else {
                &meta.title
            }
        );
        let temp = format!(".new-{id}.md");
        self.commit_change(&chapter.book_id, actor, message, move |dir| {
            let cdir = dir.join(&chapter_path);
            let mut items: Vec<(String, String)> = list_sorted(&cdir, false, ".md")
                .map_err(internal)?
                .into_iter()
                .map(|n| (n.clone(), slug_of(&n)))
                .collect();
            write_atomic(&cdir.join(&temp), &content).map_err(internal)?;
            let at = position.unwrap_or(items.len()).min(items.len());
            items.insert(at, (temp, slug));
            renumber(&cdir, &items, ".md")
        })
        .await?;
        self.sync(&chapter.book_id, &[]).await?;
        self.read_scene(&id).await
    }

    /// `ids` must name every scene of the chapter exactly once, in the wanted order.
    pub async fn reorder_scenes(
        &self,
        chapter_id: &str,
        ids: &[String],
        actor: &str,
    ) -> Result<Vec<SceneInfo>, StoreError> {
        let chapter = self.get_chapter(chapter_id).await?;
        let _guard = self.lock(&chapter.book_id).await;
        let chapter_path = self.chapter_path(chapter_id).await?;
        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT id, path FROM scenes WHERE chapter_id = ?")
                .bind(chapter_id)
                .fetch_all(&self.pool)
                .await?;
        let items = ordered_items(&rows, ids, "scene")?;
        self.commit_change(
            &chapter.book_id,
            actor,
            "Reorder scenes".to_string(),
            move |dir| renumber(&dir.join(chapter_path), &items, ".md"),
        )
        .await?;
        self.sync(&chapter.book_id, &[]).await?;
        self.list_scenes(chapter_id).await
    }

    /// Saves the scene as one commit. The client sends the whole scene; paragraphs that keep their
    /// `id` keep their identity whatever happened to their text or position. Paragraphs left out are
    /// removed (tombstoned); `merged` records which remaining passage absorbed a removed one.
    pub async fn save_scene(
        &self,
        scene_id: &str,
        save: &SaveScene,
        actor: &str,
    ) -> Result<Saved, StoreError> {
        let book_id = self.scene_row(scene_id).await?.0.book_id;
        let _guard = self.lock(&book_id).await;
        // Read again under the lock: a reorder may have moved the file while we waited.
        let (info, path) = self.scene_row(scene_id).await?;

        let (current, old) = self.load_scene(info, path.clone()).await?;
        if current.revision != save.base_revision {
            return Err(StoreError::Conflict(Box::new(current)));
        }

        let meta = build_meta(scene_id, &save.fields, &current.info.language)?;
        let texts = self.check_passages(&book_id, scene_id, save).await?;
        let pairs = self.assign_ids(save, texts).await?;
        let content = manuscript::render_scene(&meta, &pairs).map_err(internal)?;
        if old == content {
            return Ok(Saved {
                scene: current,
                commit: None,
            });
        }

        let message = format!(
            "Save scene: {}",
            if meta.title.is_empty() {
                scene_id
            } else {
                &meta.title
            }
        );
        let commit = self
            .commit_change(&book_id, actor, message, move |dir| {
                write_atomic(&dir.join(path), &content).map_err(internal)
            })
            .await?;
        let merges: Vec<(String, String)> = save
            .merged
            .iter()
            .map(|m| (m.from.clone(), m.into.clone()))
            .collect();
        self.sync(&book_id, &merges).await?;
        Ok(Saved {
            scene: self.read_scene(scene_id).await?,
            commit,
        })
    }

    /// Validates the passages of a save against the paragraph rules and the index. Returns the
    /// normalised text of each, in order.
    async fn check_passages(
        &self,
        book_id: &str,
        scene_id: &str,
        save: &SaveScene,
    ) -> Result<Vec<String>, StoreError> {
        let mut texts = Vec::with_capacity(save.passages.len());
        let mut seen: HashSet<&str> = HashSet::new();
        for (n, p) in save.passages.iter().enumerate() {
            texts.push(
                manuscript::normalize_paragraph(&p.text)
                    .map_err(|e| invalid(format!("passage {}: {e}", n + 1)))?,
            );
            let Some(id) = p.id.as_deref() else { continue };
            if !ids::is_code(id) {
                return Err(invalid(format!("passage {}: malformed id", n + 1)));
            }
            if !seen.insert(id) {
                return Err(invalid(format!("passage {}: the id is used twice", n + 1)));
            }
            let row: Option<(String, String)> = sqlx::query_as(
                "SELECT scene_id, status FROM passages WHERE id = ? AND book_id = ?",
            )
            .bind(id)
            .bind(book_id)
            .fetch_optional(&self.pool)
            .await?;
            match row {
                None => return Err(invalid(format!("passage {}: unknown id", n + 1))),
                Some((sid, status)) if status == "active" && sid != scene_id => {
                    return Err(invalid(format!(
                        "passage {}: the id is active in another scene; remove it there first",
                        n + 1
                    )));
                }
                Some(_) => {}
            }
        }
        for m in &save.merged {
            if seen.contains(m.from.as_str()) || !seen.contains(m.into.as_str()) {
                return Err(invalid(
                    "a merge must name a removed passage and a passage that stays",
                ));
            }
        }
        Ok(texts)
    }

    /// Gives every new paragraph an ID that exists nowhere yet.
    async fn assign_ids(
        &self,
        save: &SaveScene,
        texts: Vec<String>,
    ) -> Result<Vec<(String, String)>, StoreError> {
        let mut used: HashSet<String> = save.passages.iter().filter_map(|p| p.id.clone()).collect();
        let mut out = Vec::with_capacity(texts.len());
        for (p, text) in save.passages.iter().zip(texts) {
            let id = match &p.id {
                Some(id) => id.clone(),
                None => loop {
                    let candidate = ids::code();
                    let taken: Option<(i64,)> =
                        sqlx::query_as("SELECT 1 FROM passages WHERE id = ?")
                            .bind(&candidate)
                            .fetch_optional(&self.pool)
                            .await?;
                    if taken.is_none() && used.insert(candidate.clone()) {
                        break candidate;
                    }
                },
            };
            out.push((id, text));
        }
        Ok(out)
    }
}

const SCENE_SELECT_BY_CHAPTER: &str = "SELECT id, book_id, chapter_id, position, title, language, pov, \
     story_date AS date, location FROM scenes WHERE chapter_id = ? ORDER BY position";

fn non_empty(label: &str, value: &str) -> Result<String, StoreError> {
    let v = manuscript::clean_field(label, value).map_err(invalid)?;
    if v.is_empty() {
        return Err(invalid(format!("{label} cannot be empty")));
    }
    Ok(v)
}

fn optional(label: &str, value: &Option<String>) -> Result<Option<String>, StoreError> {
    match value {
        None => Ok(None),
        Some(v) => {
            let c = manuscript::clean_field(label, v).map_err(invalid)?;
            Ok(if c.is_empty() { None } else { Some(c) })
        }
    }
}

fn build_meta(id: &str, f: &SceneFields, default_language: &str) -> Result<SceneMeta, StoreError> {
    let language = f.language.as_deref().unwrap_or(default_language);
    if !manuscript::valid_language(language) {
        return Err(invalid("language must be \"ru\" or \"en\""));
    }
    Ok(SceneMeta {
        id: id.to_string(),
        title: manuscript::clean_field("title", &f.title).map_err(invalid)?,
        language: language.to_string(),
        pov: optional("pov", &f.pov)?,
        date: optional("date", &f.date)?,
        location: optional("location", &f.location)?,
    })
}

/// Turns `(id, repository path)` rows and a wanted order of IDs into `(current name, slug)` items.
/// The wanted order must contain every row exactly once.
fn ordered_items(
    rows: &[(String, String)],
    wanted: &[String],
    what: &str,
) -> Result<Vec<(String, String)>, StoreError> {
    let by_id: HashMap<&str, &str> = rows.iter().map(|(i, p)| (i.as_str(), p.as_str())).collect();
    let distinct: HashSet<&str> = wanted.iter().map(String::as_str).collect();
    if wanted.len() != rows.len()
        || distinct.len() != wanted.len()
        || wanted.iter().any(|i| !by_id.contains_key(i.as_str()))
    {
        return Err(invalid(format!(
            "the list must name every {what} exactly once"
        )));
    }
    Ok(wanted
        .iter()
        .map(|id| {
            let name = by_id[id.as_str()]
                .rsplit('/')
                .next()
                .unwrap_or_default()
                .to_string();
            let slug = slug_of(&name);
            (name, slug)
        })
        .collect())
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
