use std::path::Path;

use codex_lsp::file_search::FffFileSearch;
use codex_lsp::file_search::FileSearchKind;

#[tokio::test]
async fn finds_normal_file_from_root() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join("src")).unwrap();
    std::fs::write(tmp.path().join("src").join("main.rs"), "fn main() {}\n").unwrap();

    let search = FffFileSearch::default();
    let matches = search.search(tmp.path(), "main", 20).await;

    assert!(
        matches
            .iter()
            .any(|m| m.path.as_path() == Path::new("src").join("main.rs")),
        "expected src/main.rs in {matches:?}"
    );
}

#[tokio::test]
async fn excludes_git_internals_but_returns_regular_files() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join(".git")).unwrap();
    std::fs::write(tmp.path().join(".git").join("config"), "ignored").unwrap();
    std::fs::write(tmp.path().join("config.txt"), "visible").unwrap();

    let search = FffFileSearch::default();
    let matches = search.search(tmp.path(), "config", 20).await;

    assert!(
        matches.iter().any(|m| m.path == Path::new("config.txt")),
        "expected config.txt in {matches:?}"
    );
    assert!(
        !matches.iter().any(|m| m.path.starts_with(".git")),
        "expected .git internals excluded, got {matches:?}"
    );
}

#[tokio::test]
async fn returns_directory_matches_for_directories_with_files() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join("docs").join("guides")).unwrap();
    std::fs::write(
        tmp.path().join("docs").join("guides").join("intro.md"),
        "intro",
    )
    .unwrap();

    let search = FffFileSearch::default();
    let matches = search.search(tmp.path(), "guides", 20).await;

    assert!(
        matches.iter().any(|m| {
            m.kind == FileSearchKind::Directory
                && m.path.as_path() == Path::new("docs").join("guides")
        }),
        "expected docs/guides directory in {matches:?}"
    );
}

#[tokio::test]
async fn invalid_root_returns_no_matches() {
    let tmp = tempfile::tempdir().unwrap();
    let missing = tmp.path().join("missing");

    let search = FffFileSearch::default();
    let matches = search.search(&missing, "anything", 20).await;

    assert!(matches.is_empty());
}

#[tokio::test]
async fn reuses_cached_picker_for_repeated_root_searches() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("alpha.txt"), "alpha").unwrap();
    std::fs::write(tmp.path().join("beta.txt"), "beta").unwrap();

    let search = FffFileSearch::default();
    let first = search.search(tmp.path(), "alpha", 20).await;
    let second = search.search(tmp.path(), "beta", 20).await;

    assert!(
        first.iter().any(|m| m.path == Path::new("alpha.txt")),
        "expected alpha.txt in {first:?}"
    );
    assert!(
        second.iter().any(|m| m.path == Path::new("beta.txt")),
        "expected beta.txt in {second:?}"
    );
    assert_eq!(search.cached_root_count(), 1);
}
