//! Whole-document validation -> LSP diagnostics.
//!
//! Rules (per plan §5.4):
//! * Unknown first-line `/command` -> ERROR (only once it can't be a prefix of
//!   any known command, to avoid flagging mid-typing).
//! * `@skill` that exists in the registry -> accepted as a skill mention.
//! * `@path` that doesn't resolve to an existing file -> WARNING.

use std::path::Path;
use std::path::PathBuf;

use ropey::Rope;
use tower_lsp_server::ls_types::Diagnostic;
use tower_lsp_server::ls_types::DiagnosticSeverity;
use tower_lsp_server::ls_types::NumberOrString;

use crate::document::span_to_range;
use crate::registry::Registry;
use crate::tokens::MentionKind;
use crate::tokens::looks_like_slash_prefix;
use crate::tokens::scan_commands;
use crate::tokens::scan_mentions;

pub async fn compute(rope: &Rope, registry: &Registry, doc_dir: Option<&Path>) -> Vec<Diagnostic> {
    let text = rope.to_string();
    let mut diags = Vec::new();
    let prompt_names = registry.prompt_names();

    // Unknown commands on any line-leading slash.
    for (name, start, end) in scan_commands(&text) {
        if !registry.is_known_command(&name) && !looks_like_slash_prefix(&name, "", &prompt_names) {
            diags.push(diag(
                rope,
                start,
                end,
                DiagnosticSeverity::ERROR,
                "unknown-command",
                format!("Unknown slash command: /{name}"),
            ));
        }
    }

    // `@` mentions can be skills or files. Known skills win; everything else is
    // treated as a file reference when we know the document directory.
    for m in scan_mentions(&text) {
        match m.kind {
            MentionKind::File => {
                if registry.has_skill(&m.query) {
                    continue;
                }
                let Some(dir) = doc_dir else { continue };
                let resolved = resolve_path(dir, &m.query);
                if !tokio::fs::try_exists(&resolved).await.unwrap_or(false) {
                    diags.push(diag(
                        rope,
                        m.start,
                        m.end,
                        DiagnosticSeverity::WARNING,
                        "missing-file",
                        format!("File not found: {}", m.query),
                    ));
                }
            }
        }
    }

    diags
}

fn resolve_path(dir: &Path, raw: &str) -> PathBuf {
    let unquoted = raw.trim_matches('"');
    let p = Path::new(unquoted);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        dir.join(p)
    }
}

fn diag(
    rope: &Rope,
    start: usize,
    end: usize,
    severity: DiagnosticSeverity,
    code: &str,
    message: String,
) -> Diagnostic {
    Diagnostic {
        range: span_to_range(rope, start, end),
        severity: Some(severity),
        code: Some(NumberOrString::String(code.to_string())),
        source: Some("codex".to_string()),
        message,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::Skill;

    fn reg() -> Registry {
        Registry {
            prompts: vec![],
            skills: vec![Skill {
                name: "review".into(),
                description: None,
                display_name: None,
            }],
        }
    }

    #[tokio::test]
    async fn unknown_command_is_error() {
        let rope = Rope::from_str("/definitelynotacommand do thing");
        let d = compute(&rope, &reg(), None).await;
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].severity, Some(DiagnosticSeverity::ERROR));
    }

    #[tokio::test]
    async fn known_command_and_prefix_are_clean() {
        assert!(
            compute(&Rope::from_str("/model"), &reg(), None)
                .await
                .is_empty()
        );
        // "/mod" is a prefix of "model" -> not flagged while typing.
        assert!(
            compute(&Rope::from_str("/mod"), &reg(), None)
                .await
                .is_empty()
        );
    }

    #[tokio::test]
    async fn known_at_skill_is_not_a_missing_file_warning() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(
            compute(&Rope::from_str("use @review"), &reg(), Some(tmp.path()))
                .await
                .is_empty()
        );
    }

    #[tokio::test]
    async fn dollar_skill_is_not_a_skill_diagnostic() {
        assert!(
            compute(&Rope::from_str("use $nope please"), &reg(), None)
                .await
                .is_empty()
        );
    }

    #[tokio::test]
    async fn missing_file_is_warning() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("exists.txt"), "x").unwrap();
        let rope = Rope::from_str("@exists.txt and @nope.txt");
        let d = compute(&rope, &reg(), Some(tmp.path())).await;
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("nope.txt"));
    }
}
