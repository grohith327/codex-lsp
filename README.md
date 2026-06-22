# codex-lsp

A standalone [Language Server](https://microsoft.github.io/language-server-protocol/)
for **`.codex` files** — free-form prompt text (exactly what you'd type into the
Codex CLI input box) containing `@file` references, `/slash` commands, and
`$skill` mentions. Plug it into any LSP-capable editor for completion and
validation.

## Features (v1)

- **`@` completion** — fuzzy file-path search (reuses codex's `file-search`
  engine: ripgrep walker + nucleo matcher, respects `.gitignore`, excludes
  `.git`). The `@` menu also surfaces codex "plugins" — **skills** (inserted as
  `$name`) and **custom prompts** (inserted as `/prompts:name`) — mirroring the
  codex composer.
- **`/command` completion** — built-in slash commands and custom prompts
  (`/prompts:<name>`), with descriptions.
- **`$skill` completion** — skills discovered from `SKILL.md` files.
- **Diagnostics** — unknown commands (error), broken `@file` paths (warning),
  unknown skills (warning).

## Build

```sh
cargo build --release
# binary at target/release/codex-lsp
```

> This package vendors codex's `file-search` crate under `file-search/` and uses
> it through a local path dependency in `Cargo.toml`.

## Where prompts & skills come from

- Custom prompts: `$CODEX_HOME/prompts/*.md` (`$CODEX_HOME` defaults to `~/.codex`).
- Skills: `SKILL.md` files under the editor's workspace roots and
  `$CODEX_HOME/skills`.

Loaded once at `initialize`.

## Editor setup

See [`editors/`](editors/) for VS Code, Neovim, and Helix snippets. The
universal contract: launch `codex-lsp` over stdio and map `*.codex` to language
id `codex`.

## Architecture

```
editor ──stdio JSON-RPC──> codex-lsp
  backend.rs   LanguageServer impl (initialize, did_*, completion)
  document.rs  Rope store + UTF-16<->byte position conversion
  tokens.rs    byte-offset tokenizer (ported from codex tui chat_composer)
  fuzzy.rs     subsequence fuzzy matcher (ported from codex common)
  slash_command.rs  built-in command catalog (ported from codex tui)
  registry.rs  prompt/skill loaders + cache
  completion.rs / diagnostics.rs
        └─ reuses codex-file-search (path dep) for @ search
```

Run `cargo test` for the LSP regression suite. Run
`cargo test --manifest-path file-search/Cargo.toml` for the vendored
file-search crate tests.
