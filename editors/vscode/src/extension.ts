import * as vscode from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from "vscode-languageclient/node";

import { resolveServerCommand } from "./serverPath";

let client: LanguageClient | undefined;

export function activate(context: vscode.ExtensionContext): void {
  const configuration = vscode.workspace.getConfiguration("codexLsp");
  const command = resolveServerCommand(configuration.get<string>("serverPath"));
  const run = { command, transport: TransportKind.stdio };
  const serverOptions: ServerOptions = { run, debug: run };
  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "codex" }],
  };

  client = new LanguageClient(
    "codexLsp",
    "Codex LSP",
    serverOptions,
    clientOptions,
  );

  context.subscriptions.push(client);
  void client.start();
}

export function deactivate(): Thenable<void> | undefined {
  return client?.stop();
}
