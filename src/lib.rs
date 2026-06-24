//! codex-lsp: a standalone Language Server for `.codex` prompt files.
//!
//! Provides completion and diagnostics for the codex CLI input syntax —
//! `@file` references, `/slash` commands (incl. `/prompts:…`), and `@skill`
//! mentions — over stdio, pluggable into any LSP-capable editor.

pub mod backend;
pub mod completion;
pub mod diagnostics;
pub mod document;
pub mod file_search;
pub mod fuzzy;
pub mod registry;
pub mod slash_command;
pub mod tokens;
