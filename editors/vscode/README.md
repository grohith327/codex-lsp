# Codex LSP for VS Code

This extension registers `.codex` files and launches `codex-lsp` over stdio.

## Local development

Build the Rust server first:

```sh
cd /Users/rohith/Documents/codex-lsp
cargo build --release
mkdir -p ~/.local/bin
ln -sf /Users/rohith/Documents/codex-lsp/target/release/codex-lsp ~/.local/bin/codex-lsp
```

Install and compile the extension:

```sh
cd /Users/rohith/Documents/codex-lsp/editors/vscode
npm install
npm run compile
```

Open this folder in VS Code and press `F5` to start an Extension Development Host.
Open a `.codex` file in that host and type `@`, `/`, or `$` to trigger completions.

If the server is not on `PATH`, set `codexLsp.serverPath` to the absolute binary path:

```json
{
  "codexLsp.serverPath": "/Users/rohith/Documents/codex-lsp/target/release/codex-lsp"
}
```
