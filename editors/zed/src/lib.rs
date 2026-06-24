use zed_extension_api::{self as zed, settings::LspSettings};

const LANGUAGE_SERVER_ID: &str = "codex-lsp";

struct CodexLspExtension;

impl zed::Extension for CodexLspExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        let configured_path = LspSettings::for_worktree(LANGUAGE_SERVER_ID, worktree)
            .ok()
            .and_then(|settings| settings.binary.and_then(|binary| binary.path));
        let path_binary = worktree.which(LANGUAGE_SERVER_ID);
        let command = resolve_server_path(configured_path.as_deref(), path_binary.as_deref())?;

        Ok(zed::Command {
            command,
            args: Vec::new(),
            env: worktree.shell_env(),
        })
    }
}

fn resolve_server_path(
    configured_path: Option<&str>,
    path_binary: Option<&str>,
) -> zed::Result<String> {
    if let Some(path) = configured_path
        .map(str::trim)
        .filter(|path| !path.is_empty())
    {
        return Ok(path.to_string());
    }

    if let Some(path) = path_binary.map(str::trim).filter(|path| !path.is_empty()) {
        return Ok(path.to_string());
    }

    Err(
        "codex-lsp binary was not found. Build it with `cargo build --release`, then either add `target/release/codex-lsp` to your PATH, link it as `~/.local/bin/codex-lsp`, or set `lsp.codex-lsp.binary.path` in Zed settings."
            .to_string(),
    )
}

zed::register_extension!(CodexLspExtension);
