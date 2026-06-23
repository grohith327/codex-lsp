const DEFAULT_SERVER_COMMAND = "codex-lsp";

export function resolveServerCommand(configuredPath: string | undefined): string {
  const trimmed = configuredPath?.trim();
  return trimmed && trimmed.length > 0 ? trimmed : DEFAULT_SERVER_COMMAND;
}
