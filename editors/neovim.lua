-- Neovim 0.11+ : attach codex-lsp to *.codex files.
-- Ensure the `codex-lsp` binary is on your PATH (or use an absolute path in cmd).

vim.filetype.add({ extension = { codex = "codex" } })

vim.lsp.config["codex"] = {
  cmd = { "codex-lsp" },
  filetypes = { "codex" },
  root_markers = { ".git" },
}

vim.lsp.enable("codex")
