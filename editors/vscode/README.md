# Codex LSP for VS Code

This extension registers `.codex` files and launches `codex-lsp` over stdio.

## Install for regular use

Build the Rust server and put it somewhere stable:

```sh
cd ../..
cargo build --release
mkdir -p ~/.local/bin
ln -sf "$PWD/target/release/codex-lsp" ~/.local/bin/codex-lsp
```

Package and install the VS Code extension:

```sh
cd editors/vscode
npm install
npm run compile
npm install -g @vscode/vsce
vsce package
code --install-extension codex-lsp-vscode-0.0.1.vsix
```

Restart VS Code. After that, opening any `.codex` file should automatically
activate this extension and start `codex-lsp`.

If completions do not appear, VS Code may not be inheriting your shell `PATH`.
Set the server path globally in VS Code settings:

```json
{
  "codexLsp.serverPath": "/Users/rohith/Documents/codex-lsp/target/release/codex-lsp"
}
```

Then reload VS Code and open a `.codex` file. Typing `@`, `/`, or `$` should
trigger completions from the language server.

## Local development

For extension development, run:

```sh
npm install
npm run compile
code .
```

Press `F5` in VS Code to start an Extension Development Host.
