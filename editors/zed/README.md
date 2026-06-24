# Codex LSP for Zed

This extension registers `.codex` files in Zed and launches `codex-lsp` over
stdio.

## Install as a dev extension

Build the Rust server and put it somewhere stable:

```sh
cd ../..
cargo build --release
mkdir -p ~/.local/bin
ln -sf "$PWD/target/release/codex-lsp" ~/.local/bin/codex-lsp
```

Install the Zed wrapper:

1. Open Zed.
2. Run `zed: install dev extension` from the command palette.
3. Select this `editors/zed` directory.
4. Open a `.codex` file.

Typing `@`, `/`, or `$` should trigger completions from the language server.

If completions do not appear, Zed may not be inheriting your shell `PATH`. Set
the server path in Zed settings:

```json
{
  "lsp": {
    "codex-lsp": {
      "binary": {
        "path": "/Users/rohith/Documents/codex-lsp/target/release/codex-lsp"
      }
    }
  }
}
```

Then reload Zed and open a `.codex` file.

## Local development

Check the extension crate:

```sh
cargo test --manifest-path editors/zed/Cargo.toml
cargo check --manifest-path editors/zed/Cargo.toml --target wasm32-wasip1
```

For Extension Gallery publishing, add this repo to
`zed-industries/extensions` as a submodule and point the registry entry at
`editors/zed`.
