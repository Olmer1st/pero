use super::*;
use crate::db;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// A throw-away data folder with a real SQLite file and a store on top of it.
struct Env {
    dir: PathBuf,
    store: Arc<Store>,
}

impl Env {
    async fn new() -> Env {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("pero-store-test-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let pool = db::open(&dir.join("pero.db")).await.unwrap();
        let store = Store::new(pool, dir.join("books")).unwrap();
        Env { dir, store }
    }

    fn repo(&self, book: &Book) -> PathBuf {
        self.dir.join("books").join(&book.id)
    }

    fn commits(&self, book: &Book) -> usize {
        git::commit_count(&self.repo(book))
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

const A: &str = "Над Доном вставало солнце — холодное, чужое.";
const B: &str = "— Ну что, казак, — сказал он, — поедем?";
const C: &str = "«Ёлочки» и ёж.\nВторая строка абзаца.";

fn fields(title: &str) -> SceneFields {
    SceneFields {
        title: title.into(),
        ..Default::default()
    }
}

fn save(scene: &Scene, passages: &[(Option<&str>, &str)]) -> SaveScene {
    SaveScene {
        base_revision: scene.revision.clone(),
        fields: fields(&scene.info.title),
        passages: passages
            .iter()
            .map(|(id, text)| PassageIn {
                id: id.map(String::from),
                text: text.to_string(),
            })
            .collect(),
        merged: vec![],
    }
}

fn ids_of(scene: &Scene) -> Vec<String> {
    scene
        .passages
        .iter()
        .map(|p| p.id.clone().unwrap())
        .collect()
}

fn texts_of(scene: &Scene) -> Vec<&str> {
    scene.passages.iter().map(|p| p.text.as_str()).collect()
}

async fn book_with_scene(env: &Env) -> (Book, Chapter, Scene) {
    let book = env
        .store
        .create_book("Тихий Дон", "ru", "tester")
        .await
        .unwrap();
    let chapter = env
        .store
        .create_chapter(&book.id, "Глава первая", None, "tester")
        .await
        .unwrap();
    let scene = env
        .store
        .create_scene(&chapter.id, &fields("Утро"), None, "tester")
        .await
        .unwrap();
    (book, chapter, scene)
}

async fn first_save(env: &Env, scene: &Scene) -> Scene {
    env.store
        .save_scene(
            &scene.info.id,
            &save(scene, &[(None, A), (None, B), (None, C)]),
            "tester",
        )
        .await
        .unwrap()
        .scene
}

async fn passage_rows(env: &Env, scene_id: &str) -> Vec<(String, String, i64, Option<String>)> {
    sqlx::query_as("SELECT id, status, position, merged_into FROM passages WHERE scene_id = ? ORDER BY position, id")
        .bind(scene_id)
        .fetch_all(&env.store.pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn a_book_is_a_git_repository_with_one_commit() {
    let env = Env::new().await;
    let book = env
        .store
        .create_book("Тихий Дон", "ru", "tester")
        .await
        .unwrap();
    assert!(book.id.starts_with("bk_") && book.head_commit.is_some());
    let repo = env.repo(&book);
    assert!(repo.join(".git").is_dir());
    let toml = fs::read_to_string(repo.join("book.toml")).unwrap();
    assert!(toml.contains("Тихий Дон") && toml.contains(&book.id));
    assert_eq!(git::log_messages(&repo), ["tester|Create book: Тихий Дон"]);
    assert_eq!(env.store.list_books().await.unwrap().len(), 1);
    assert!(env.store.create_book("", "ru", "t").await.is_err());
    assert!(env.store.create_book("X", "de", "t").await.is_err());
}

#[tokio::test]
async fn chapters_and_scenes_get_numbered_files_and_one_commit_each() {
    let env = Env::new().await;
    let (book, chapter, scene) = book_with_scene(&env).await;
    assert_eq!(env.commits(&book), 3);
    let repo = env.repo(&book);
    assert!(
        repo.join("chapters/001-glava-pervaya/chapter.toml")
            .is_file()
    );
    assert!(
        repo.join("chapters/001-glava-pervaya/001-utro.md")
            .is_file()
    );
    assert_eq!(scene.info.chapter_id, chapter.id);
    assert_eq!(scene.info.language, "ru");
    assert!(scene.passages.is_empty());
    // inserting at the front renumbers the others
    let second = env
        .store
        .create_chapter(&book.id, "Глава вторая", Some(0), "tester")
        .await
        .unwrap();
    assert!(
        repo.join("chapters/001-glava-vtoraya").is_dir()
            && repo.join("chapters/002-glava-pervaya").is_dir()
    );
    assert_eq!(second.position, 0);
    assert_eq!(
        env.store.get_chapter(&chapter.id).await.unwrap().position,
        1
    );
    assert_eq!(env.commits(&book), 4);
    // the scene is still found after its chapter folder was renamed
    assert_eq!(
        env.store.read_scene(&scene.info.id).await.unwrap().info.id,
        scene.info.id
    );
}

#[tokio::test]
async fn saving_assigns_ids_writes_markers_and_indexes_the_passages() {
    let env = Env::new().await;
    let (book, _, scene) = book_with_scene(&env).await;
    let saved = env
        .store
        .save_scene(
            &scene.info.id,
            &save(&scene, &[(None, A), (None, B), (None, C)]),
            "tester",
        )
        .await
        .unwrap();
    assert!(saved.commit.is_some());
    let ids = ids_of(&saved.scene);
    assert_eq!(ids.len(), 3);
    assert!(ids.iter().all(|i| ids::is_code(i)));
    assert_eq!(texts_of(&saved.scene), [A, B, C]);
    let file = fs::read_to_string(
        env.repo(&book)
            .join("chapters/001-glava-pervaya/001-utro.md"),
    )
    .unwrap();
    for (id, text) in ids.iter().zip([A, B, C]) {
        assert!(
            file.contains(&format!("{text} <!-- p:{id} -->\n")),
            "{file}"
        );
    }
    let rows = passage_rows(&env, &scene.info.id).await;
    assert_eq!(rows.iter().map(|r| r.0.clone()).collect::<Vec<_>>(), ids);
    assert!(rows.iter().all(|r| r.1 == "active"));
    let hash: (String,) = sqlx::query_as("SELECT text_hash FROM passages WHERE id = ?")
        .bind(&ids[0])
        .fetch_one(&env.store.pool)
        .await
        .unwrap();
    assert_eq!(hash.0, manuscript::text_hash(A));
}

#[tokio::test]
async fn russian_text_reaches_the_file_and_git_byte_for_byte() {
    let env = Env::new().await;
    let (book, _, scene) = book_with_scene(&env).await;
    let saved = first_save(&env, &scene).await;
    let path = "chapters/001-glava-pervaya/001-utro.md";
    let on_disk = fs::read(env.repo(&book).join(path)).unwrap();
    assert_eq!(manuscript::sha256_hex(&on_disk), saved.revision);
    let in_git = std::process::Command::new("git")
        .args(["show", &format!("HEAD:{path}")])
        .current_dir(env.repo(&book))
        .output()
        .unwrap()
        .stdout;
    assert_eq!(in_git, on_disk);
    let text = String::from_utf8(on_disk).unwrap();
    assert!(
        text.contains("— Ну что, казак, — сказал он, — поедем?")
            && text.contains("«Ёлочки» и ёж.\nВторая строка")
    );
}

#[tokio::test]
async fn editing_text_keeps_the_passage_id() {
    let env = Env::new().await;
    let (book, _, scene) = book_with_scene(&env).await;
    let v1 = first_save(&env, &scene).await;
    let ids = ids_of(&v1);
    let before = env.commits(&book);
    let edited = "Над Доном вставало солнце — тусклое.";
    let v2 = env
        .store
        .save_scene(
            &scene.info.id,
            &save(
                &v1,
                &[
                    (Some(&ids[0]), edited),
                    (Some(&ids[1]), B),
                    (Some(&ids[2]), C),
                ],
            ),
            "tester",
        )
        .await
        .unwrap();
    assert_eq!(ids_of(&v2.scene), ids);
    assert_eq!(texts_of(&v2.scene)[0], edited);
    assert_eq!(env.commits(&book), before + 1);
    let hash: (String,) = sqlx::query_as("SELECT text_hash FROM passages WHERE id = ?")
        .bind(&ids[0])
        .fetch_one(&env.store.pool)
        .await
        .unwrap();
    assert_eq!(hash.0, manuscript::text_hash(edited));
}

#[tokio::test]
async fn reordering_paragraphs_keeps_every_id_with_its_text_in_one_commit() {
    let env = Env::new().await;
    let (book, _, scene) = book_with_scene(&env).await;
    let v1 = first_save(&env, &scene).await;
    let ids = ids_of(&v1);
    let before = env.commits(&book);
    let v2 = env
        .store
        .save_scene(
            &scene.info.id,
            &save(
                &v1,
                &[(Some(&ids[2]), C), (Some(&ids[0]), A), (Some(&ids[1]), B)],
            ),
            "tester",
        )
        .await
        .unwrap();
    assert_eq!(
        ids_of(&v2.scene),
        [ids[2].clone(), ids[0].clone(), ids[1].clone()]
    );
    assert_eq!(texts_of(&v2.scene), [C, A, B]);
    assert_eq!(env.commits(&book), before + 1);
    let rows = passage_rows(&env, &scene.info.id).await;
    assert_eq!(
        rows.iter().map(|r| r.0.clone()).collect::<Vec<_>>(),
        ids_of(&v2.scene)
    );
    assert!(rows.iter().all(|r| r.1 == "active"));
}

#[tokio::test]
async fn splitting_merging_removing_and_reviving_passages() {
    let env = Env::new().await;
    let (_, _, scene) = book_with_scene(&env).await;
    let v1 = first_save(&env, &scene).await;
    let ids = ids_of(&v1);

    // split the first paragraph: the first half keeps the id, the second half is new
    let v2 = env
        .store
        .save_scene(
            &scene.info.id,
            &save(
                &v1,
                &[
                    (Some(&ids[0]), "Над Доном вставало солнце —"),
                    (None, "холодное, чужое."),
                    (Some(&ids[1]), B),
                    (Some(&ids[2]), C),
                ],
            ),
            "tester",
        )
        .await
        .unwrap()
        .scene;
    let ids2 = ids_of(&v2);
    assert_eq!(ids2[0], ids[0]);
    assert!(!ids.contains(&ids2[1]));

    // merge the new half into the first again: the dropped one is tombstoned and points at the survivor
    let mut merge = save(
        &v2,
        &[(Some(&ids[0]), A), (Some(&ids[1]), B), (Some(&ids[2]), C)],
    );
    merge.merged = vec![Merge {
        from: ids2[1].clone(),
        into: ids[0].clone(),
    }];
    let v3 = env
        .store
        .save_scene(&scene.info.id, &merge, "tester")
        .await
        .unwrap()
        .scene;
    assert_eq!(ids_of(&v3), ids);
    let removed: (String, Option<String>, Option<String>) =
        sqlx::query_as("SELECT status, merged_into, removed_at FROM passages WHERE id = ?")
            .bind(&ids2[1])
            .fetch_one(&env.store.pool)
            .await
            .unwrap();
    assert_eq!(removed.0, "removed");
    assert_eq!(removed.1.as_deref(), Some(ids[0].as_str()));
    assert!(removed.2.is_some());

    // plain removal, then revival of the same id (undo)
    let v4 = env
        .store
        .save_scene(
            &scene.info.id,
            &save(&v3, &[(Some(&ids[0]), A), (Some(&ids[2]), C)]),
            "tester",
        )
        .await
        .unwrap()
        .scene;
    let gone: (String, Option<String>) =
        sqlx::query_as("SELECT status, merged_into FROM passages WHERE id = ?")
            .bind(&ids[1])
            .fetch_one(&env.store.pool)
            .await
            .unwrap();
    assert_eq!(gone, ("removed".to_string(), None));
    let v5 = env
        .store
        .save_scene(
            &scene.info.id,
            &save(
                &v4,
                &[(Some(&ids[0]), A), (Some(&ids[1]), B), (Some(&ids[2]), C)],
            ),
            "tester",
        )
        .await
        .unwrap()
        .scene;
    assert_eq!(ids_of(&v5), ids);
    let back: (String, Option<String>) =
        sqlx::query_as("SELECT status, removed_at FROM passages WHERE id = ?")
            .bind(&ids[1])
            .fetch_one(&env.store.pool)
            .await
            .unwrap();
    assert_eq!(back, ("active".to_string(), None));
}

#[tokio::test]
async fn a_passage_can_move_to_another_scene_after_it_was_removed_from_the_first() {
    let env = Env::new().await;
    let (_, chapter, scene1) = book_with_scene(&env).await;
    let scene2 = env
        .store
        .create_scene(&chapter.id, &fields("Вечер"), None, "tester")
        .await
        .unwrap();
    let v1 = first_save(&env, &scene1).await;
    let ids = ids_of(&v1);

    // claiming an id that is still active elsewhere is refused
    let err = env
        .store
        .save_scene(
            &scene2.info.id,
            &save(&scene2, &[(Some(&ids[1]), B)]),
            "tester",
        )
        .await
        .unwrap_err();
    assert!(
        matches!(err, StoreError::Invalid(ref m) if m.contains("another scene")),
        "{err}"
    );

    // remove it from the first scene, then the second scene may take it with the same id
    env.store
        .save_scene(
            &scene1.info.id,
            &save(&v1, &[(Some(&ids[0]), A), (Some(&ids[2]), C)]),
            "tester",
        )
        .await
        .unwrap();
    let v = env
        .store
        .save_scene(
            &scene2.info.id,
            &save(&scene2, &[(Some(&ids[1]), B)]),
            "tester",
        )
        .await
        .unwrap()
        .scene;
    assert_eq!(ids_of(&v), [ids[1].clone()]);
    let row: (String, String) =
        sqlx::query_as("SELECT scene_id, status FROM passages WHERE id = ?")
            .bind(&ids[1])
            .fetch_one(&env.store.pool)
            .await
            .unwrap();
    assert_eq!(row, (scene2.info.id.clone(), "active".to_string()));
}

#[tokio::test]
async fn a_save_based_on_an_old_revision_is_refused_and_changes_nothing() {
    let env = Env::new().await;
    let (book, _, scene) = book_with_scene(&env).await;
    let v1 = first_save(&env, &scene).await;
    let stale = save(&scene, &[(None, "чужая правка")]); // based on the empty scene, not on v1
    let before = env.commits(&book);
    match env.store.save_scene(&scene.info.id, &stale, "tester").await {
        Err(StoreError::Conflict(current)) => {
            assert_eq!(current.revision, v1.revision);
            assert_eq!(texts_of(&current), [A, B, C]);
        }
        other => panic!("expected a conflict, got {other:?}"),
    }
    assert_eq!(env.commits(&book), before);
    assert_eq!(
        env.store.read_scene(&scene.info.id).await.unwrap().revision,
        v1.revision
    );
}

#[tokio::test]
async fn two_saves_from_the_same_revision_let_exactly_one_through() {
    let env = Env::new().await;
    let (_, _, scene) = book_with_scene(&env).await;
    let a = save(&scene, &[(None, "первая")]);
    let b = save(&scene, &[(None, "вторая")]);
    let (s1, s2) = (env.store.clone(), env.store.clone());
    let id = scene.info.id.clone();
    let id2 = id.clone();
    let t1 = tokio::spawn(async move { s1.save_scene(&id, &a, "one").await });
    let t2 = tokio::spawn(async move { s2.save_scene(&id2, &b, "two").await });
    let (r1, r2) = (t1.await.unwrap(), t2.await.unwrap());
    let ok = [r1.is_ok(), r2.is_ok()];
    assert_eq!(ok.iter().filter(|x| **x).count(), 1, "{r1:?} {r2:?}");
    let conflicts = [&r1, &r2]
        .iter()
        .filter(|r| matches!(r, Err(StoreError::Conflict(_))))
        .count();
    assert_eq!(conflicts, 1);
}

#[tokio::test]
async fn saving_the_same_content_makes_no_commit() {
    let env = Env::new().await;
    let (book, _, scene) = book_with_scene(&env).await;
    let v1 = first_save(&env, &scene).await;
    let ids = ids_of(&v1);
    let before = env.commits(&book);
    let again = env
        .store
        .save_scene(
            &scene.info.id,
            &save(
                &v1,
                &[(Some(&ids[0]), A), (Some(&ids[1]), B), (Some(&ids[2]), C)],
            ),
            "tester",
        )
        .await
        .unwrap();
    assert!(again.commit.is_none());
    assert_eq!(again.scene.revision, v1.revision);
    assert_eq!(env.commits(&book), before);
}

#[tokio::test]
async fn invalid_saves_are_rejected_before_anything_is_written() {
    let env = Env::new().await;
    let (book, _, scene) = book_with_scene(&env).await;
    let v1 = first_save(&env, &scene).await;
    let ids = ids_of(&v1);
    let before = env.commits(&book);
    let path = env
        .repo(&book)
        .join("chapters/001-glava-pervaya/001-utro.md");
    let file_before = fs::read(&path).unwrap();

    let known = Some(ids[0].as_str());
    let cases: Vec<(&str, SaveScene)> = vec![
        ("blank line", save(&v1, &[(None, "раз\n\nдва")])),
        ("empty", save(&v1, &[(None, "  ")])),
        (
            "marker in text",
            save(&v1, &[(None, "текст <!-- p:k7m2x9qa -->")]),
        ),
        ("heading", save(&v1, &[(None, "# Заголовок")])),
        ("unknown id", save(&v1, &[(Some("zzzzzzzz"), "текст")])),
        ("malformed id", save(&v1, &[(Some("не-код"), "текст")])),
        ("duplicate id", save(&v1, &[(known, "раз"), (known, "два")])),
        (
            "bad language",
            SaveScene {
                fields: SceneFields {
                    title: "Утро".into(),
                    language: Some("de".into()),
                    ..Default::default()
                },
                ..save(&v1, &[(known, A)])
            },
        ),
        (
            "merge from a staying passage",
            SaveScene {
                merged: vec![Merge {
                    from: ids[0].clone(),
                    into: ids[1].clone(),
                }],
                ..save(&v1, &[(known, A), (Some(&ids[1]), B)])
            },
        ),
    ];
    for (name, request) in cases {
        let err = env
            .store
            .save_scene(&scene.info.id, &request, "tester")
            .await
            .unwrap_err();
        assert!(matches!(err, StoreError::Invalid(_)), "{name}: {err}");
    }
    assert_eq!(env.commits(&book), before);
    assert_eq!(fs::read(&path).unwrap(), file_before);
}

#[tokio::test]
async fn scene_fields_are_replaced_and_empty_values_are_dropped() {
    let env = Env::new().await;
    let (book, _, scene) = book_with_scene(&env).await;
    let mut request = save(&scene, &[(None, A)]);
    request.fields = SceneFields {
        title: "Утро на хуторе".into(),
        language: Some("en".into()),
        pov: Some(" Григорий ".into()),
        date: Some("1914-06-01".into()),
        location: Some("   ".into()),
    };
    let saved = env
        .store
        .save_scene(&scene.info.id, &request, "tester")
        .await
        .unwrap()
        .scene;
    assert_eq!(saved.info.title, "Утро на хуторе");
    assert_eq!(saved.info.language, "en");
    assert_eq!(saved.info.pov.as_deref(), Some("Григорий"));
    assert_eq!(saved.info.date.as_deref(), Some("1914-06-01"));
    assert_eq!(saved.info.location, None);
    let row: (String, Option<String>) =
        sqlx::query_as("SELECT title, story_date FROM scenes WHERE id = ?")
            .bind(&scene.info.id)
            .fetch_one(&env.store.pool)
            .await
            .unwrap();
    assert_eq!(
        row,
        ("Утро на хуторе".to_string(), Some("1914-06-01".to_string()))
    );
    assert!(git::log_messages(&env.repo(&book))[0].contains("Save scene: Утро на хуторе"));
}

#[tokio::test]
async fn a_repository_with_uncommitted_changes_refuses_new_saves() {
    let env = Env::new().await;
    let (book, _, scene) = book_with_scene(&env).await;
    let stray = env.repo(&book).join("stray.txt");
    fs::write(&stray, "left by hand").unwrap();
    let err = env
        .store
        .save_scene(&scene.info.id, &save(&scene, &[(None, A)]), "tester")
        .await
        .unwrap_err();
    assert!(matches!(err, StoreError::Dirty), "{err}");
    assert!(stray.exists(), "the owner's file must not be touched");
    fs::remove_file(&stray).unwrap();
    env.store
        .save_scene(&scene.info.id, &save(&scene, &[(None, A)]), "tester")
        .await
        .unwrap();
}

#[tokio::test]
async fn a_failed_change_is_rolled_back() {
    let env = Env::new().await;
    let (book, _, _) = book_with_scene(&env).await;
    let before = env.commits(&book);
    let err = env
        .store
        .commit_change(&book.id, "tester", "never".into(), |dir| {
            fs::write(dir.join("half-done.txt"), "x").unwrap();
            fs::remove_file(dir.join("book.toml")).unwrap();
            Err(StoreError::Internal("boom".into()))
        })
        .await
        .unwrap_err();
    assert!(matches!(err, StoreError::Internal(_)));
    let repo = env.repo(&book);
    assert!(!repo.join("half-done.txt").exists() && repo.join("book.toml").exists());
    assert_eq!(env.commits(&book), before);
    assert!(git::is_clean(&repo, "tester").unwrap());
}

#[tokio::test]
async fn reordering_scenes_and_chapters_keeps_ids_and_makes_one_commit() {
    let env = Env::new().await;
    let (book, chapter, s1) = book_with_scene(&env).await;
    // two scenes with the same title (same slug) swap places: the index must not trip over the names
    let s2 = env
        .store
        .create_scene(&chapter.id, &fields("Утро"), None, "tester")
        .await
        .unwrap();
    let s3 = env
        .store
        .create_scene(&chapter.id, &fields("Вечер"), None, "tester")
        .await
        .unwrap();
    let v = first_save(&env, &s1).await;
    let before = env.commits(&book);
    let order = vec![s3.info.id.clone(), s2.info.id.clone(), s1.info.id.clone()];
    let scenes = env
        .store
        .reorder_scenes(&chapter.id, &order, "tester")
        .await
        .unwrap();
    assert_eq!(
        scenes.iter().map(|s| s.id.clone()).collect::<Vec<_>>(),
        order
    );
    assert_eq!(env.commits(&book), before + 1);
    let repo = env.repo(&book).join("chapters/001-glava-pervaya");
    assert!(
        repo.join("001-vecher.md").is_file()
            && repo.join("002-utro.md").is_file()
            && repo.join("003-utro.md").is_file()
    );
    // the scene and its passages are the same after the move
    let moved = env.store.read_scene(&s1.info.id).await.unwrap();
    assert_eq!(ids_of(&moved), ids_of(&v));
    assert_eq!(
        moved.revision, v.revision,
        "a rename does not change the file"
    );
    assert_eq!(moved.info.position, 2);

    for bad in [
        vec![s1.info.id.clone()],
        vec![s1.info.id.clone(), s1.info.id.clone(), s2.info.id.clone()],
        vec![
            s1.info.id.clone(),
            s2.info.id.clone(),
            "sc_00000000".to_string(),
        ],
    ] {
        assert!(matches!(
            env.store.reorder_scenes(&chapter.id, &bad, "tester").await,
            Err(StoreError::Invalid(_))
        ));
    }
    assert_eq!(env.commits(&book), before + 1);

    let c2 = env
        .store
        .create_chapter(&book.id, "Глава вторая", None, "tester")
        .await
        .unwrap();
    let chapters = env
        .store
        .reorder_chapters(&book.id, &[c2.id.clone(), chapter.id.clone()], "tester")
        .await
        .unwrap();
    assert_eq!(
        chapters.iter().map(|c| c.id.clone()).collect::<Vec<_>>(),
        [c2.id, chapter.id.clone()]
    );
    assert_eq!(
        env.store.read_scene(&s1.info.id).await.unwrap().revision,
        v.revision
    );
    assert!(
        env.repo(&book)
            .join("chapters/002-glava-pervaya/003-utro.md")
            .is_file()
    );
}

#[tokio::test]
async fn titles_can_be_changed_with_one_commit_each() {
    let env = Env::new().await;
    let (book, chapter, _) = book_with_scene(&env).await;
    let before = env.commits(&book);
    let b = env
        .store
        .update_book_title(&book.id, "Новая книга", "tester")
        .await
        .unwrap();
    let c = env
        .store
        .update_chapter_title(&chapter.id, "Начало", "tester")
        .await
        .unwrap();
    assert_eq!(
        (b.title.as_str(), c.title.as_str()),
        ("Новая книга", "Начало")
    );
    assert_eq!(env.commits(&book), before + 2);
    assert!(
        env.repo(&book).join("chapters/001-glava-pervaya").is_dir(),
        "folder names do not follow titles"
    );
}

#[tokio::test]
async fn the_index_can_be_rebuilt_from_the_repository() {
    let env = Env::new().await;
    let (book, chapter, scene) = book_with_scene(&env).await;
    let v = first_save(&env, &scene).await;

    // a second store with an empty database over the same book folders
    let pool = db::open(&env.dir.join("other.db")).await.unwrap();
    let other = Store::new(pool, env.dir.join("books")).unwrap();
    assert!(
        other.read_scene(&scene.info.id).await.is_err(),
        "unknown before the rebuild"
    );
    let report = other.reindex_book(&book.id).await.unwrap();
    assert_eq!(report, SyncReport { unmarked: 0 });
    let rebuilt = other.read_scene(&scene.info.id).await.unwrap();
    assert_eq!(ids_of(&rebuilt), ids_of(&v));
    assert_eq!(rebuilt.revision, v.revision);
    assert_eq!(
        other.list_chapters(&book.id).await.unwrap()[0].id,
        chapter.id
    );
    assert_eq!(other.get_book(&book.id).await.unwrap().title, "Тихий Дон");
}

#[tokio::test]
async fn paragraphs_written_outside_pero_are_reported_and_get_ids_on_the_next_save() {
    let env = Env::new().await;
    let (book, _, scene) = book_with_scene(&env).await;
    let v = first_save(&env, &scene).await;
    let path = env
        .repo(&book)
        .join("chapters/001-glava-pervaya/001-utro.md");
    let mut text = fs::read_to_string(&path).unwrap();
    text.push_str("\nАбзац, добавленный вручную.\n");
    fs::write(&path, text).unwrap();
    git::commit_all(&env.repo(&book), "owner", "Manual edit").unwrap();

    assert_eq!(
        env.store.reindex_book(&book.id).await.unwrap(),
        SyncReport { unmarked: 1 }
    );
    let current = env.store.read_scene(&scene.info.id).await.unwrap();
    assert_eq!(current.passages.len(), 4);
    assert_eq!(current.passages[3].id, None);
    assert_ne!(current.revision, v.revision);

    let ids = ids_of(&v);
    let req = save(
        &current,
        &[
            (Some(&ids[0]), A),
            (Some(&ids[1]), B),
            (Some(&ids[2]), C),
            (None, "Абзац, добавленный вручную."),
        ],
    );
    let saved = env
        .store
        .save_scene(&scene.info.id, &req, "tester")
        .await
        .unwrap()
        .scene;
    assert!(saved.passages.iter().all(|p| p.id.is_some()));
    assert_eq!(&ids_of(&saved)[..3], &ids[..]);
}

#[tokio::test]
async fn unknown_or_malformed_ids_are_not_found() {
    let env = Env::new().await;
    let (_, _, scene) = book_with_scene(&env).await;
    for id in ["sc_00000000", "../../etc/passwd", "", "bk_00000000"] {
        assert!(
            matches!(env.store.read_scene(id).await, Err(StoreError::NotFound(_))),
            "{id}"
        );
    }
    assert!(matches!(
        env.store.get_book("bk_00000000").await,
        Err(StoreError::NotFound(_))
    ));
    assert!(matches!(
        env.store.list_chapters("nope").await,
        Err(StoreError::NotFound(_))
    ));
    assert!(env.store.read_scene(&scene.info.id).await.is_ok());
}

fn md(name: &str, content: &str) -> ImportFile {
    ImportFile {
        name: name.into(),
        content: content.into(),
    }
}

const CHAPTER_1: &str = "# Глава первая\n\n## Утро\n\nНад Доном вставало солнце — холодное, чужое.\n\n— Ну что, казак, — сказал он, — поедем?\n\n* * *\n\n«Ёлочки» и ёж.\nВторая строка абзаца.\n\n## Вечер\n\n„Лапки“ и… многоточие.\n";
const CHAPTER_2: &str = "# Глава вторая\n\nОдна сцена без заголовка.\n\n— Да.\n— Нет.\n";

#[tokio::test]
async fn an_imported_manuscript_is_one_commit_with_stable_ids_everywhere() {
    let env = Env::new().await;
    let files = [md("02.md", CHAPTER_2), md("01.md", CHAPTER_1)];
    let summary = env
        .store
        .import_book(
            "Тихий Дон",
            "ru",
            ParagraphMode::BlankLine,
            &files,
            "tester",
        )
        .await
        .unwrap();
    assert_eq!(
        (summary.chapters, summary.scenes, summary.passages),
        (2, 4, 6)
    );
    assert!(summary.titles_from_filename.is_empty());
    let repo = env.repo(&summary.book);
    assert_eq!(
        git::log_messages(&repo),
        ["tester|Import book: Тихий Дон (2 chapters)"]
    );
    assert!(git::is_clean(&repo, "tester").unwrap());

    let chapters = env.store.list_chapters(&summary.book.id).await.unwrap();
    assert_eq!(
        chapters
            .iter()
            .map(|c| c.title.as_str())
            .collect::<Vec<_>>(),
        ["Глава первая", "Глава вторая"]
    );
    let scenes = env.store.list_scenes(&chapters[0].id).await.unwrap();
    assert_eq!(
        scenes.iter().map(|s| s.title.as_str()).collect::<Vec<_>>(),
        ["Утро", "", "Вечер"]
    );
    assert!(
        repo.join("chapters/001-glava-pervaya/001-utro.md")
            .is_file()
    );
    assert!(
        repo.join("chapters/001-glava-pervaya/002-scene.md")
            .is_file()
    );

    let first = env.store.read_scene(&scenes[0].id).await.unwrap();
    assert_eq!(
        texts_of(&first),
        [
            "Над Доном вставало солнце — холодное, чужое.",
            "— Ну что, казак, — сказал он, — поедем?"
        ]
    );
    let (count,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM passages WHERE book_id = ? AND status = 'active'")
            .bind(&summary.book.id)
            .fetch_one(&env.store.pool)
            .await
            .unwrap();
    let (distinct,): (i64,) =
        sqlx::query_as("SELECT count(DISTINCT id) FROM passages WHERE book_id = ?")
            .bind(&summary.book.id)
            .fetch_one(&env.store.pool)
            .await
            .unwrap();
    assert_eq!((count, distinct), (6, 6));
}

#[tokio::test]
async fn import_then_export_is_byte_identical_to_the_normalised_original() {
    let env = Env::new().await;
    let messy_2 = CHAPTER_2.replace('\n', "\r\n") + "\r\n\r\n\r\n";
    let files = [md("01.md", CHAPTER_1), md("02.md", &messy_2)];
    let summary = env
        .store
        .import_book("Книга", "ru", ParagraphMode::BlankLine, &files, "tester")
        .await
        .unwrap();
    let exported = env.store.export_book(&summary.book.id).await.unwrap();
    assert_eq!(exported.len(), 2);
    assert_eq!(exported[0].content, CHAPTER_1);
    assert_eq!(exported[1].content, import::normalize_markdown(&messy_2));
    for (original, out) in files.iter().zip(&exported) {
        let check = import::check_file(original, &out.content, None);
        assert!(check.same_text && check.canonical, "{check:?}");
    }
}

#[tokio::test]
async fn a_refused_import_leaves_no_book_behind() {
    let env = Env::new().await;
    let files = [
        md("01.md", CHAPTER_1),
        md("02.md", "# Г\n\nтекст\n\n### нельзя\n"),
    ];
    match env
        .store
        .import_book("Книга", "ru", ParagraphMode::BlankLine, &files, "tester")
        .await
    {
        Err(StoreError::ImportRejected(issues)) => {
            assert_eq!((issues[0].file.as_str(), issues[0].line), ("02.md", 5));
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
    assert!(env.store.list_books().await.unwrap().is_empty());
    assert_eq!(fs::read_dir(env.dir.join("books")).unwrap().count(), 0);
    let (n,): (i64,) = sqlx::query_as("SELECT count(*) FROM passages")
        .fetch_one(&env.store.pool)
        .await
        .unwrap();
    assert_eq!(n, 0);
    assert!(
        env.store
            .import_book("", "ru", ParagraphMode::BlankLine, &files[..1], "t")
            .await
            .is_err()
    );
    assert!(
        env.store
            .import_book("К", "xx", ParagraphMode::BlankLine, &files[..1], "t")
            .await
            .is_err()
    );
}

#[tokio::test]
async fn an_imported_book_can_be_edited_and_keeps_its_ids() {
    let env = Env::new().await;
    let summary = env
        .store
        .import_book(
            "Книга",
            "ru",
            ParagraphMode::BlankLine,
            &[md("01.md", CHAPTER_2)],
            "tester",
        )
        .await
        .unwrap();
    let chapter = env
        .store
        .list_chapters(&summary.book.id)
        .await
        .unwrap()
        .remove(0);
    let info = env.store.list_scenes(&chapter.id).await.unwrap().remove(0);
    let scene = env.store.read_scene(&info.id).await.unwrap();
    let ids = ids_of(&scene);
    let edited = "Одна сцена, теперь с правкой.";
    let req = save(
        &scene,
        &[(Some(&ids[0]), edited), (Some(&ids[1]), "— Да.\n— Нет.")],
    );
    let saved = env
        .store
        .save_scene(&info.id, &req, "tester")
        .await
        .unwrap();
    assert_eq!(ids_of(&saved.scene), ids);
    assert_eq!(env.commits(&summary.book), 2);
}

#[tokio::test]
async fn single_line_paragraph_mode_gives_one_passage_per_line() {
    let env = Env::new().await;
    let text = "# Глава\n\n— Привет, — сказал он.\n— Привет.\n— Как дела?\n";
    let blank = env
        .store
        .import_book(
            "А",
            "ru",
            ParagraphMode::BlankLine,
            &[md("a.md", text)],
            "t",
        )
        .await
        .unwrap();
    let line = env
        .store
        .import_book("Б", "ru", ParagraphMode::Line, &[md("a.md", text)], "t")
        .await
        .unwrap();
    assert_eq!((blank.passages, line.passages), (1, 3));
}

#[tokio::test]
async fn a_large_manuscript_imports_in_one_go() {
    let env = Env::new().await;
    let paragraph =
        "Над Доном вставало солнце — холодное, чужое, и казаки, не торопясь, седлали коней.";
    let files: Vec<ImportFile> = (1..=20)
        .map(|c| {
            let scenes: String = (1..=10)
                .map(|s| format!("## Сцена {s}\n\n{}", format!("{paragraph}\n\n").repeat(10)))
                .collect();
            md(&format!("{c:02}.md"), &format!("# Глава {c}\n\n{scenes}"))
        })
        .collect();
    let started = std::time::Instant::now();
    let summary = env
        .store
        .import_book("Большая", "ru", ParagraphMode::BlankLine, &files, "tester")
        .await
        .unwrap();
    eprintln!(
        "imported {} passages in {:?}",
        summary.passages,
        started.elapsed()
    );
    assert_eq!(
        (summary.chapters, summary.scenes, summary.passages),
        (20, 200, 2000)
    );
    assert_eq!(env.commits(&summary.book), 1);
    let exported = env.store.export_book(&summary.book.id).await.unwrap();
    for (original, out) in files.iter().zip(&exported) {
        assert!(import::check_file(original, &out.content, None).canonical);
    }
}
