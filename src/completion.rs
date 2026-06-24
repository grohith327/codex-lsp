//! Build LSP completion items for the token under the cursor.

use std::path::Path;

use ropey::Rope;
use tower_lsp_server::ls_types::CompletionItem;
use tower_lsp_server::ls_types::CompletionItemKind;
use tower_lsp_server::ls_types::CompletionList;
use tower_lsp_server::ls_types::CompletionResponse;
use tower_lsp_server::ls_types::CompletionTextEdit;
use tower_lsp_server::ls_types::Position;
use tower_lsp_server::ls_types::Range;
use tower_lsp_server::ls_types::TextEdit;

use crate::document::position_to_byte;
use crate::document::span_to_range;
use crate::file_search::FffFileSearch;
use crate::fuzzy::fuzzy_match;
use crate::registry::Registry;
use crate::slash_command::PROMPTS_CMD_PREFIX;
use crate::slash_command::built_in_slash_commands;
use crate::tokens::CompletionContext;
use crate::tokens::TokenSpan;
use crate::tokens::completion_context;

const MAX_FILE_RESULTS: usize = 50;

/// Compute completions for `pos` in `rope`. `search_root` is where `@file`
/// queries are resolved (the document's directory, ideally).
pub async fn complete(
    rope: &Rope,
    pos: Position,
    registry: &Registry,
    search_root: Option<&Path>,
    file_search: &FffFileSearch,
) -> Option<CompletionResponse> {
    let text = rope.to_string();
    let cursor = position_to_byte(rope, pos)?;
    let ctx = completion_context(&text, cursor, &registry.prompt_names())?;

    let items = match ctx {
        CompletionContext::Command {
            content_start,
            content_end,
            query,
        } => {
            let range = span_to_range(rope, content_start, content_end);
            command_items(&query, registry, range)
        }
        CompletionContext::Skill(span) => {
            let range = span_to_range(rope, span.content_start, span.end);
            skill_items(&span.query, registry, range)
        }
        CompletionContext::File(span) => {
            file_items(rope, &span, registry, search_root, file_search).await
        }
    };

    Some(CompletionResponse::List(CompletionList {
        is_incomplete: true,
        items,
    }))
}

fn command_items(query: &str, registry: &Registry, range: Range) -> Vec<CompletionItem> {
    let mut scored: Vec<(i32, CompletionItem)> = Vec::new();

    for (cmd_name, cmd) in built_in_slash_commands() {
        if let Some((_, score)) = fuzzy_match(cmd_name, query) {
            scored.push((
                score,
                CompletionItem {
                    label: format!("/{cmd_name}"),
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some(cmd.description().to_string()),
                    filter_text: Some(cmd_name.to_string()),
                    sort_text: Some(sort_key(score)),
                    text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                        range,
                        new_text: format!("{cmd_name} "),
                    })),
                    ..Default::default()
                },
            ));
        }
    }

    for prompt in &registry.prompts {
        let candidate = format!("{PROMPTS_CMD_PREFIX}:{}", prompt.name);
        if let Some((_, score)) = fuzzy_match(&candidate, query) {
            scored.push((
                score,
                CompletionItem {
                    label: format!("/{candidate}"),
                    kind: Some(CompletionItemKind::VALUE),
                    detail: prompt.description.clone(),
                    filter_text: Some(candidate.clone()),
                    sort_text: Some(sort_key(score)),
                    text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                        range,
                        new_text: format!("{candidate} "),
                    })),
                    ..Default::default()
                },
            ));
        }
    }

    scored.sort_by_key(|(s, _)| *s);
    scored.into_iter().map(|(_, item)| item).collect()
}

fn skill_items(query: &str, registry: &Registry, range: Range) -> Vec<CompletionItem> {
    let mut scored: Vec<(i32, CompletionItem)> = Vec::new();
    for skill in &registry.skills {
        if let Some(score) = skill_match_score(skill, query) {
            scored.push((
                score,
                CompletionItem {
                    label: skill_label(skill),
                    kind: Some(CompletionItemKind::VALUE),
                    detail: skill.description.clone(),
                    filter_text: Some(skill_filter_text(skill)),
                    sort_text: Some(sort_key(score)),
                    text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                        range,
                        new_text: skill.name.clone(),
                    })),
                    ..Default::default()
                },
            ));
        }
    }
    scored.sort_by_key(|(s, _)| *s);
    scored.into_iter().map(|(_, item)| item).collect()
}

/// `@`-context completions. Codex surfaces "plugins" — skills and custom
/// prompts — in the `@` menu alongside files, so we list all three: skills
/// first, then prompts, then file matches. Selecting a skill/prompt rewrites
/// the whole `@token` into its canonical reference (`$skill` / `/prompts:name`),
/// so those edits span the leading `@` and their filter text carries it too.
async fn file_items(
    rope: &Rope,
    span: &TokenSpan,
    registry: &Registry,
    search_root: Option<&Path>,
    file_search: &FffFileSearch,
) -> Vec<CompletionItem> {
    let mut items = Vec::new();
    let whole = span_to_range(rope, span.start, span.end);

    // Skills (e.g. the `pdf-processing` "plugin") -> inserted as `$name`.
    for skill in &registry.skills {
        if let Some(score) = skill_match_score(skill, &span.query) {
            items.push(CompletionItem {
                label: skill_label(skill),
                kind: Some(CompletionItemKind::EVENT),
                detail: skill
                    .description
                    .clone()
                    .or_else(|| Some("codex skill".to_string())),
                filter_text: Some(format!("@{}", skill_filter_text(skill))),
                // Prefix "0" so skills sort to the very top of the `@` menu.
                sort_text: Some(format!("0{}", sort_key(score))),
                text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                    range: whole,
                    new_text: format!("${} ", skill.name),
                })),
                ..Default::default()
            });
        }
    }

    // Custom prompts -> inserted as `/prompts:<name>`.
    for prompt in &registry.prompts {
        if let Some((_, score)) = fuzzy_match(&prompt.name, &span.query) {
            items.push(CompletionItem {
                label: format!("/{PROMPTS_CMD_PREFIX}:{}", prompt.name),
                kind: Some(CompletionItemKind::REFERENCE),
                detail: prompt
                    .description
                    .clone()
                    .or_else(|| Some("codex prompt".to_string())),
                filter_text: Some(format!("@{}", prompt.name)),
                sort_text: Some(format!("1{}", sort_key(score))),
                text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                    range: whole,
                    new_text: format!("/{PROMPTS_CMD_PREFIX}:{} ", prompt.name),
                })),
                ..Default::default()
            });
        }
    }

    // File matches (skipped for an empty query or when no search root is known).
    if !span.query.is_empty()
        && let Some(root) = search_root.map(Path::to_path_buf)
    {
        let matches = file_search
            .search(&root, &span.query, MAX_FILE_RESULTS)
            .await;
        let range = span_to_range(rope, span.content_start, span.end);
        for m in matches {
            let path = m.path.to_string_lossy().into_owned();
            items.push(CompletionItem {
                label: path.clone(),
                kind: Some(CompletionItemKind::FILE),
                filter_text: Some(path.clone()),
                // Higher fff score = better, so negate for ascending sort.
                // Prefix "2" so files sort below skills and prompts.
                sort_text: Some(format!("2{}{}", sort_key(m.score.saturating_neg()), path)),
                text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                    range,
                    new_text: maybe_quote(&path),
                })),
                ..Default::default()
            });
        }
    }

    items
}

fn skill_match_score(skill: &crate::registry::Skill, query: &str) -> Option<i32> {
    let name_score = fuzzy_match(&skill.name, query).map(|(_, score)| score);
    let display_score = skill
        .display_name
        .as_deref()
        .and_then(|display_name| fuzzy_match(display_name, query))
        .map(|(_, score)| score);
    match (name_score, display_score) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(score), None) | (None, Some(score)) => Some(score),
        (None, None) => None,
    }
}

fn skill_label(skill: &crate::registry::Skill) -> String {
    skill
        .display_name
        .clone()
        .unwrap_or_else(|| format!("${}", skill.name))
}

fn skill_filter_text(skill: &crate::registry::Skill) -> String {
    skill
        .display_name
        .clone()
        .unwrap_or_else(|| skill.name.clone())
}

fn maybe_quote(path: &str) -> String {
    if path.chars().any(char::is_whitespace) && !path.contains('"') {
        format!("\"{path}\"")
    } else {
        path.to_string()
    }
}

/// Map a fuzzy score (smaller = better, may be negative) to an ascending
/// lexicographically-sortable key.
fn sort_key(score: i32) -> String {
    format!("{:020}", score as i64 + 2_000_000_000)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_search::FffFileSearch;

    fn reg() -> Registry {
        Registry {
            prompts: vec![crate::registry::CustomPrompt {
                name: "deploy".into(),
                description: Some("ship".into()),
            }],
            skills: vec![crate::registry::Skill {
                name: "review".into(),
                description: Some("review code".into()),
                display_name: None,
            }],
        }
    }

    #[tokio::test]
    async fn command_completion_lists_builtins_and_prompts() {
        let rope = Rope::from_str("/mod");
        let search = FffFileSearch::default();
        let resp = complete(&rope, Position::new(0, 4), &reg(), None, &search)
            .await
            .expect("response");
        let CompletionResponse::List(list) = resp else {
            panic!("expected list")
        };
        assert!(list.is_incomplete);
        assert!(list.items.iter().any(|i| i.label == "/model"));
    }

    #[tokio::test]
    async fn prompt_completion_via_prefix() {
        let rope = Rope::from_str("/prompts:dep");
        let search = FffFileSearch::default();
        let resp = complete(&rope, Position::new(0, 12), &reg(), None, &search)
            .await
            .expect("response");
        let CompletionResponse::List(list) = resp else {
            panic!("expected list")
        };
        assert!(list.items.iter().any(|i| i.label == "/prompts:deploy"));
    }

    #[tokio::test]
    async fn at_context_includes_prompts() {
        let rope = Rope::from_str("@dep");
        let search = FffFileSearch::default();
        let resp = complete(&rope, Position::new(0, 4), &reg(), None, &search)
            .await
            .expect("response");
        let CompletionResponse::List(list) = resp else {
            panic!("expected list")
        };
        let item = list
            .items
            .iter()
            .find(|i| i.label == "/prompts:deploy")
            .expect("prompt should appear in @ context");
        match item.text_edit.as_ref().expect("edit") {
            CompletionTextEdit::Edit(e) => assert_eq!(e.new_text, "/prompts:deploy "),
            _ => panic!("expected edit"),
        }
    }

    #[tokio::test]
    async fn at_context_includes_skills() {
        // Typing `@rev` must surface the skill (codex's "plugin") as `$review`.
        let rope = Rope::from_str("@rev");
        let search = FffFileSearch::default();
        let resp = complete(&rope, Position::new(0, 4), &reg(), None, &search)
            .await
            .expect("response");
        let CompletionResponse::List(list) = resp else {
            panic!("expected list")
        };
        let item = list
            .items
            .iter()
            .find(|i| i.label == "$review")
            .expect("skill should appear in @ context");
        match item.text_edit.as_ref().expect("edit") {
            CompletionTextEdit::Edit(e) => assert_eq!(e.new_text, "$review "),
            _ => panic!("expected edit"),
        }
    }

    #[tokio::test]
    async fn git_dir_excluded_from_file_search() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".git")).unwrap();
        std::fs::write(tmp.path().join(".git").join("config"), "x").unwrap();
        std::fs::write(tmp.path().join("config.txt"), "x").unwrap();

        let rope = Rope::from_str("@config");
        let search = FffFileSearch::default();
        let resp = complete(
            &rope,
            Position::new(0, 7),
            &reg(),
            Some(tmp.path()),
            &search,
        )
        .await
        .expect("response");
        let CompletionResponse::List(list) = resp else {
            panic!("expected list")
        };
        let files: Vec<String> = list
            .items
            .iter()
            .filter(|i| i.kind == Some(CompletionItemKind::FILE))
            .map(|i| i.label.clone())
            .collect();
        assert!(
            files.iter().any(|f| f.contains("config.txt")),
            "expected config.txt in {files:?}"
        );
        assert!(
            !files.iter().any(|f| f.contains(".git")),
            "expected .git excluded, got {files:?}"
        );
    }

    #[tokio::test]
    async fn skill_completion() {
        let rope = Rope::from_str("use $rev");
        let search = FffFileSearch::default();
        let resp = complete(&rope, Position::new(0, 8), &reg(), None, &search)
            .await
            .expect("response");
        let CompletionResponse::List(list) = resp else {
            panic!("expected list")
        };
        let item = list
            .items
            .iter()
            .find(|i| i.label == "$review")
            .expect("review");
        match item.text_edit.as_ref().expect("edit") {
            CompletionTextEdit::Edit(e) => assert_eq!(e.new_text, "review"),
            _ => panic!("expected edit"),
        }
    }

    #[test]
    fn quoting() {
        assert_eq!(maybe_quote("a/b.rs"), "a/b.rs");
        assert_eq!(maybe_quote("a b.rs"), "\"a b.rs\"");
    }

    #[tokio::test]
    async fn file_completion_quotes_paths_with_spaces() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("space dir")).unwrap();
        std::fs::write(tmp.path().join("space dir").join("target file.rs"), "x").unwrap();

        let rope = Rope::from_str("@target");
        let search = FffFileSearch::default();
        let resp = complete(
            &rope,
            Position::new(0, 7),
            &reg(),
            Some(tmp.path()),
            &search,
        )
        .await
        .expect("response");
        let CompletionResponse::List(list) = resp else {
            panic!("expected list")
        };
        let item = list
            .items
            .iter()
            .find(|i| i.label.contains("target file.rs"))
            .expect("file with spaces should appear");
        match item.text_edit.as_ref().expect("edit") {
            CompletionTextEdit::Edit(e) => {
                assert_eq!(e.new_text, "\"space dir/target file.rs\"");
            }
            _ => panic!("expected edit"),
        }
    }

    #[tokio::test]
    async fn at_context_sorts_skills_prompts_before_files() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("review-notes.md"), "x").unwrap();
        std::fs::write(tmp.path().join("deploy-notes.md"), "x").unwrap();

        let rope = Rope::from_str("@rev");
        let search = FffFileSearch::default();
        let resp = complete(
            &rope,
            Position::new(0, 4),
            &reg(),
            Some(tmp.path()),
            &search,
        )
        .await
        .expect("response");
        let CompletionResponse::List(list) = resp else {
            panic!("expected list")
        };

        let skill_pos = list
            .items
            .iter()
            .position(|i| i.label == "$review")
            .expect("skill");
        let file_pos = list
            .items
            .iter()
            .position(|i| i.kind == Some(CompletionItemKind::FILE))
            .expect("file");
        assert!(skill_pos < file_pos, "items were {:?}", list.items);

        let rope = Rope::from_str("@dep");
        let resp = complete(
            &rope,
            Position::new(0, 4),
            &reg(),
            Some(tmp.path()),
            &search,
        )
        .await
        .expect("response");
        let CompletionResponse::List(list) = resp else {
            panic!("expected list")
        };
        let prompt_pos = list
            .items
            .iter()
            .position(|i| i.label == "/prompts:deploy")
            .expect("prompt");
        let file_pos = list
            .items
            .iter()
            .position(|i| i.kind == Some(CompletionItemKind::FILE))
            .expect("file");
        assert!(prompt_pos < file_pos, "items were {:?}", list.items);
    }
}
