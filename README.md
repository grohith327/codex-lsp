# codex-lsp

A standalone [Language Server](https://microsoft.github.io/language-server-protocol/)
for **`.codex` files** — free-form prompt text (exactly what you'd type into the
Codex CLI input box) containing `@file` references, `/slash` commands, and
`@skill` mentions. Plug it into any LSP-capable editor for completion and
validation.

<img width="1520" height="1080" alt="codex-lsp-demo" src="https://github.com/user-attachments/assets/d2481f5f-75f8-42a1-98b8-fc2d53a35627" />

## Add the binary to your PATH

Build the release binary:

```sh
cargo build --release
```

Create a stable link somewhere on your `PATH`:

```sh
mkdir -p ~/.local/bin
ln -sf "$PWD/target/release/codex-lsp" ~/.local/bin/codex-lsp
```

If `~/.local/bin` is not already on your `PATH`, add it to your shell config:

```sh
export PATH="$HOME/.local/bin:$PATH"
```

Verify that your shell can find the server:

```sh
which codex-lsp
```

You can also skip the link and use the absolute path to the binary in your
editor config:

```sh
/path/to/codex-lsp/target/release/codex-lsp
```

## LSP Configuration

Add `codex-lsp` to your favourite code editor by following the setup below.
The server speaks LSP over stdio, so your editor needs to launch the
`codex-lsp` binary for `*.codex` files.

### <img src="https://cdn.jsdelivr.net/gh/devicons/devicon@latest/icons/vscode/vscode-original.svg" width="20" height="20" alt="VS Code logo" style="vertical-align: text-bottom;"> VS Code

The VS Code extension lives in `editors/vscode`. First make sure the
`codex-lsp` binary is available by following the setup below, then package and
install the VSIX:

```sh
cd editors/vscode
npm install
npm run compile
npm install -g @vscode/vsce
vsce package
code --install-extension codex-lsp-vscode-0.0.1.vsix
```

Restart VS Code after installing the extension. Opening any `.codex` file should
then start `codex-lsp` automatically.

If VS Code cannot find the server because it was launched outside your shell,
set `codexLsp.serverPath` to the absolute path of the release binary:

```json
{
  "codexLsp.serverPath": "/path/to/codex-lsp/target/release/codex-lsp"
}
```

See `editors/vscode/README.md` for the full VS Code workflow.

### <img src="https://cdn.simpleicons.org/zedindustries/084CCF" width="20" height="20" alt="Zed logo" style="vertical-align: text-bottom;"> Zed

The Zed extension lives in `editors/zed`. First build `codex-lsp` and put it on
your `PATH`, then install the Zed wrapper as a dev extension:

```sh
cargo build --release
mkdir -p ~/.local/bin
ln -sf "$PWD/target/release/codex-lsp" ~/.local/bin/codex-lsp
```

In Zed, run `zed: install dev extension` from the command palette and select
the `editors/zed` directory. Opening any `.codex` file should then start
`codex-lsp` automatically.

If Zed cannot find the server because it was launched outside your shell, set
the language server binary path in Zed settings:

```json
{
  "lsp": {
    "codex-lsp": {
      "binary": {
        "path": "/path/to/codex-lsp/target/release/codex-lsp"
      }
    }
  }
}
```

See `editors/zed/README.md` for the full Zed workflow.

### <img src="https://cdn.simpleicons.org/neovim/57A143" width="20" height="20" alt="Neovim logo" style="vertical-align: text-bottom;"> Neovim

For Neovim 0.11+, add this to your Neovim config, for example in
`~/.config/nvim/init.lua`:

```lua
-- Neovim 0.11+ : attach codex-lsp to *.codex files.
-- Ensure the `codex-lsp` binary is on your PATH (or use an absolute path in cmd).

vim.filetype.add({ extension = { codex = "codex" } })

vim.lsp.config["codex"] = {
  cmd = { "codex-lsp" },
  filetypes = { "codex" },
  root_markers = { ".git" },
}

vim.lsp.enable("codex")
```

If you did not create the `~/.local/bin/codex-lsp` link, point `cmd` directly at
the release binary:

```lua
vim.lsp.config["codex"] = {
  cmd = { "/path/to/codex-lsp/target/release/codex-lsp" },
  filetypes = { "codex" },
  root_markers = { ".git" },
}
```

Open a `*.codex` file and run `:LspInfo` to confirm that `codex-lsp` is
attached.
