//! Byte-offset token detection for `.codex` files.
//!
//! Ported from `codex-rs/tui/src/bottom_pane/chat_composer.rs`
//! (`current_prefixed_token`, `slash_command_under_cursor`,
//! `looks_like_slash_prefix`, `clamp_to_char_boundary`). The TUI operates on a
//! `TextArea`; here we operate on `(text: &str, cursor_byte: usize)` and track
//! byte spans so the LSP can build `TextEdit` ranges.
//!
//! Whitespace is `char::is_whitespace()` (NOT ASCII-only), matching the CLI
//! exactly — e.g. U+3000 full-width space delimits a token.

use crate::fuzzy::fuzzy_match;
use crate::slash_command::PROMPTS_CMD_PREFIX;
use crate::slash_command::built_in_slash_commands;

/// A prefixed token (`@…` or `$…`) located in the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenSpan {
    /// Text after the prefix character (e.g. `@foo` -> `foo`).
    pub query: String,
    /// Byte offset of the prefix character.
    pub start: usize,
    /// Byte offset just past the end of the token.
    pub end: usize,
    /// Byte offset just after the prefix character (`start + prefix.len_utf8()`).
    pub content_start: usize,
}

/// What the cursor is currently positioned to complete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionContext {
    /// First-line slash command. `content_*` bound the name (excluding `/`).
    Command {
        query: String,
        content_start: usize,
        content_end: usize,
    },
    File(TokenSpan),
    Skill(TokenSpan),
}

/// Adjust `pos` to the nearest valid char boundary at or before it.
pub fn clamp_to_char_boundary(text: &str, pos: usize) -> usize {
    let mut p = pos.min(text.len());
    if p < text.len() && !text.is_char_boundary(p) {
        p = text
            .char_indices()
            .map(|(i, _)| i)
            .take_while(|&i| i <= pos)
            .last()
            .unwrap_or(0);
    }
    p
}

/// Extract the prefixed token (`@`/`$`) the cursor is positioned on, if any.
pub fn current_prefixed_token(
    text: &str,
    cursor_offset: usize,
    prefix: char,
    allow_empty: bool,
) -> Option<TokenSpan> {
    let safe_cursor = clamp_to_char_boundary(text, cursor_offset);
    let before_cursor = &text[..safe_cursor];
    let after_cursor = &text[safe_cursor..];

    let at_whitespace = if safe_cursor < text.len() {
        text[safe_cursor..]
            .chars()
            .next()
            .map(char::is_whitespace)
            .unwrap_or(false)
    } else {
        false
    };

    // Left candidate: token containing the cursor position.
    let start_left = before_cursor
        .char_indices()
        .rfind(|(_, c)| c.is_whitespace())
        .map(|(idx, c)| idx + c.len_utf8())
        .unwrap_or(0);
    let end_left_rel = after_cursor
        .char_indices()
        .find(|(_, c)| c.is_whitespace())
        .map(|(idx, _)| idx)
        .unwrap_or(after_cursor.len());
    let end_left = safe_cursor + end_left_rel;
    let token_left = if start_left < end_left {
        Some(&text[start_left..end_left])
    } else {
        None
    };

    // Right candidate: token immediately after any whitespace from the cursor.
    let ws_len_right: usize = after_cursor
        .chars()
        .take_while(|c| c.is_whitespace())
        .map(char::len_utf8)
        .sum();
    let start_right = safe_cursor + ws_len_right;
    let end_right_rel = text[start_right..]
        .char_indices()
        .find(|(_, c)| c.is_whitespace())
        .map(|(idx, _)| idx)
        .unwrap_or(text.len() - start_right);
    let end_right = start_right + end_right_rel;
    let token_right = if start_right < end_right {
        Some(&text[start_right..end_right])
    } else {
        None
    };

    let prefix_len = prefix.len_utf8();
    let prefix_str = prefix.to_string();

    let make = |tok: &str, start: usize, end: usize| TokenSpan {
        query: tok[prefix_len..].to_string(),
        start,
        end,
        content_start: start + prefix_len,
    };

    let left_match = token_left
        .filter(|t| t.starts_with(prefix))
        .map(|t| (t, start_left, end_left));
    let right_match = token_right
        .filter(|t| t.starts_with(prefix))
        .map(|t| (t, start_right, end_right));

    if at_whitespace {
        if let Some((t, s, e)) = right_match {
            return Some(make(t, s, e));
        }
        if token_left.is_some_and(|t| t == prefix_str) {
            return allow_empty.then(|| TokenSpan {
                query: String::new(),
                start: start_left,
                end: end_left,
                content_start: start_left + prefix_len,
            });
        }
        return left_match.map(|(t, s, e)| make(t, s, e));
    }
    if after_cursor.starts_with(prefix) {
        return right_match.or(left_match).map(|(t, s, e)| make(t, s, e));
    }
    left_match.or(right_match).map(|(t, s, e)| make(t, s, e))
}

pub fn current_at_token(text: &str, cursor: usize) -> Option<TokenSpan> {
    current_prefixed_token(text, cursor, '@', false)
}

pub fn current_skill_token(text: &str, cursor: usize) -> Option<TokenSpan> {
    current_prefixed_token(text, cursor, '$', true)
}

/// If the cursor is within a slash command on the first line, return
/// `(name, rest)` where `name` excludes the leading `/`.
pub fn slash_command_under_cursor(first_line: &str, cursor: usize) -> Option<(&str, &str)> {
    if !first_line.starts_with('/') {
        return None;
    }
    let name_start = 1usize;
    let name_end = first_line[name_start..]
        .find(char::is_whitespace)
        .map(|idx| name_start + idx)
        .unwrap_or_else(|| first_line.len());

    if cursor > name_end {
        return None;
    }

    let name = &first_line[name_start..name_end];
    let rest_start = first_line[name_end..]
        .find(|c: char| !c.is_whitespace())
        .map(|idx| name_end + idx)
        .unwrap_or(name_end);
    let rest = &first_line[rest_start..];
    Some((name, rest))
}

/// Whether the typed slash name looks like a valid prefix for some command or
/// custom prompt. Empty names only count when nothing follows the `/`.
pub fn looks_like_slash_prefix(name: &str, rest_after_name: &str, prompt_names: &[String]) -> bool {
    if name.is_empty() {
        return rest_after_name.is_empty();
    }
    let builtin_match = built_in_slash_commands()
        .into_iter()
        .any(|(cmd_name, _)| fuzzy_match(cmd_name, name).is_some());
    if builtin_match {
        return true;
    }
    let prompt_prefix = format!("{PROMPTS_CMD_PREFIX}:");
    prompt_names
        .iter()
        .any(|n| fuzzy_match(&format!("{prompt_prefix}{n}"), name).is_some())
}

/// Classify what the cursor should complete, mirroring the composer precedence:
/// `@`/`$` tokens take priority over the first-line slash command.
pub fn completion_context(
    text: &str,
    cursor: usize,
    prompt_names: &[String],
) -> Option<CompletionContext> {
    let file = current_at_token(text, cursor);
    let skill = current_skill_token(text, cursor);

    if file.is_none() && skill.is_none() {
        // Slash commands are recognized at the start of ANY line. (The CLI
        // composer only checks the first line because the whole input is one
        // command, but a `.codex` file is multi-line prose, so a `/command` is
        // useful at the head of any line.)
        let line_start = text[..cursor].rfind('\n').map(|i| i + 1).unwrap_or(0);
        let line_end = text[cursor..]
            .find('\n')
            .map(|i| cursor + i)
            .unwrap_or(text.len());
        let line = &text[line_start..line_end];
        let cursor_in_line = cursor - line_start;
        if let Some((name, rest)) = slash_command_under_cursor(line, cursor_in_line)
            && looks_like_slash_prefix(name, rest, prompt_names)
        {
            let content_start = line_start + 1;
            let content_end = content_start + name.len();
            return Some(CompletionContext::Command {
                query: name.to_string(),
                content_start,
                content_end,
            });
        }
        return None;
    }
    if let Some(span) = skill {
        return Some(CompletionContext::Skill(span));
    }
    file.map(CompletionContext::File)
}

// ---- Whole-document scanning (for diagnostics) ----

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MentionKind {
    File,
    Skill,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mention {
    pub kind: MentionKind,
    pub query: String,
    pub start: usize,
    pub end: usize,
}

/// Find every non-empty `@`/`$` mention in the document (whitespace-delimited).
pub fn scan_mentions(text: &str) -> Vec<Mention> {
    let mut out = Vec::new();
    let mut token_start: Option<usize> = None;
    for (idx, ch) in text.char_indices() {
        if ch.is_whitespace() {
            if let Some(s) = token_start.take() {
                push_mention(text, s, idx, &mut out);
            }
        } else if token_start.is_none() {
            token_start = Some(idx);
        }
    }
    if let Some(s) = token_start {
        push_mention(text, s, text.len(), &mut out);
    }
    out
}

fn push_mention(text: &str, start: usize, end: usize, out: &mut Vec<Mention>) {
    let tok = &text[start..end];
    let Some(first) = tok.chars().next() else {
        return;
    };
    let kind = match first {
        '@' => MentionKind::File,
        '$' => MentionKind::Skill,
        _ => return,
    };
    let content_start = start + first.len_utf8();
    if content_start >= end {
        return; // lone prefix, not a mention
    }
    out.push(Mention {
        kind,
        query: text[content_start..end].to_string(),
        start,
        end,
    });
}

/// Find every line-leading slash command and its byte span (covering `/name`).
/// Only lines that begin with `/` are considered, so mid-sentence slashes
/// (e.g. `and/or`) are never treated as commands.
pub fn scan_commands(text: &str) -> Vec<(String, usize, usize)> {
    let mut out = Vec::new();
    let mut offset = 0usize;
    for line in text.split_inclusive('\n') {
        if line.starts_with('/') {
            let name_start = 1usize;
            let name_end = line[name_start..]
                .find(char::is_whitespace)
                .map(|idx| name_start + idx)
                .unwrap_or(line.len());
            let name = &line[name_start..name_end];
            if !name.is_empty() {
                out.push((name.to_string(), offset, offset + name_end));
            }
        }
        offset += line.len();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(text: &str, cursor: usize, prefix: char, allow_empty: bool) -> Option<String> {
        current_prefixed_token(text, cursor, prefix, allow_empty).map(|t| t.query)
    }

    #[test]
    fn at_token_basic() {
        // cursor at end of "@foo"
        assert_eq!(q("@foo", 4, '@', false).as_deref(), Some("foo"));
    }

    #[test]
    fn at_token_span_covers_prefix() {
        let span = current_at_token("hello @foo bar", 10).expect("token");
        assert_eq!(span.query, "foo");
        assert_eq!(&"hello @foo bar"[span.start..span.end], "@foo");
        assert_eq!(span.content_start, span.start + 1);
    }

    #[test]
    fn no_token_in_plain_text() {
        assert_eq!(q("hello world", 5, '@', false), None);
    }

    #[test]
    fn skill_lone_dollar_allows_empty() {
        assert_eq!(q("$", 1, '$', true).as_deref(), Some(""));
        // '@' (allow_empty=false) at whitespace boundary would not, but a lone
        // trailing prefix at end-of-text returns empty per the source logic.
    }

    #[test]
    fn second_token_selected_by_cursor() {
        let text = "@a @b";
        // Cursor inside "@a" (on the 'a') selects "a".
        assert_eq!(q(text, 1, '@', false).as_deref(), Some("a"));
        // Cursor at end selects "b".
        assert_eq!(q(text, 5, '@', false).as_deref(), Some("b"));
        // Cursor on the whitespace boundary prefers the following token (the
        // right candidate) when it carries the prefix — matches the composer.
        assert_eq!(q(text, 2, '@', false).as_deref(), Some("b"));
    }

    #[test]
    fn full_width_space_delimits() {
        // U+3000 is whitespace; "@foo" then full-width space then "bar".
        let text = "@foo\u{3000}bar";
        let span = current_at_token(text, 4).expect("token");
        assert_eq!(span.query, "foo");
    }

    #[test]
    fn unicode_cursor_midtoken() {
        let text = "@café";
        let span = current_at_token(text, text.len()).expect("token");
        assert_eq!(span.query, "café");
    }

    #[test]
    fn slash_command_detected_first_line_only() {
        assert_eq!(slash_command_under_cursor("/model", 3), Some(("model", "")));
        assert_eq!(slash_command_under_cursor("hello", 3), None);
        // cursor past the name
        assert_eq!(slash_command_under_cursor("/model foo", 9), None);
    }

    #[test]
    fn looks_like_prefix_matches_builtin() {
        assert!(looks_like_slash_prefix("mod", "", &[]));
        assert!(!looks_like_slash_prefix("zzzzz", "", &[]));
        assert!(looks_like_slash_prefix("", "", &[]));
        assert!(!looks_like_slash_prefix("", "extra", &[]));
    }

    #[test]
    fn looks_like_prefix_matches_prompt() {
        let prompts = vec!["deploy".to_string()];
        assert!(looks_like_slash_prefix("prompts:dep", "", &prompts));
    }

    #[test]
    fn completion_context_command() {
        let ctx = completion_context("/mod", 4, &[]).expect("ctx");
        match ctx {
            CompletionContext::Command {
                query,
                content_start,
                content_end,
            } => {
                assert_eq!(query, "mod");
                assert_eq!((content_start, content_end), (1, 4));
            }
            other => panic!("expected command, got {other:?}"),
        }
    }

    #[test]
    fn completion_context_at_wins_on_first_line() {
        // An @token under cursor beats slash command context.
        let ctx = completion_context("/model @sr", 10, &[]).expect("ctx");
        assert!(matches!(ctx, CompletionContext::File(_)));
    }

    #[test]
    fn scan_mentions_finds_all() {
        let text = "see @src/main.rs and use $review here";
        let m = scan_mentions(text);
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].kind, MentionKind::File);
        assert_eq!(m[0].query, "src/main.rs");
        assert_eq!(&text[m[0].start..m[0].end], "@src/main.rs");
        assert_eq!(m[1].kind, MentionKind::Skill);
        assert_eq!(m[1].query, "review");
    }

    #[test]
    fn scan_mentions_ignores_lone_prefix() {
        assert!(scan_mentions("a @ b $ c").is_empty());
    }

    #[test]
    fn scan_commands_finds_line_leading_slashes() {
        // First line and a later line; the span covers "/name".
        let text = "/bogus arg\nplain text\n/model";
        let cmds = scan_commands(text);
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0], ("bogus".to_string(), 0, 6));
        let (name, start, end) = &cmds[2 - 1];
        assert_eq!(name, "model");
        assert_eq!(&text[*start..*end], "/model");
    }

    #[test]
    fn scan_commands_ignores_midline_and_empty() {
        assert!(scan_commands("use and/or here").is_empty());
        assert!(scan_commands("/").is_empty());
    }

    #[test]
    fn slash_command_completion_on_later_line() {
        // The cursor is on line 2's "/mod" — must still produce a command.
        let text = "review this\n/mod";
        let ctx = completion_context(text, text.len(), &[]).expect("ctx");
        match ctx {
            CompletionContext::Command {
                query,
                content_start,
                content_end,
            } => {
                assert_eq!(query, "mod");
                assert_eq!(&text[content_start - 1..content_end], "/mod");
            }
            other => panic!("expected command, got {other:?}"),
        }
    }
}
