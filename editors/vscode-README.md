# VS Code integration

A minimal VS Code extension is needed to launch the server and register the
`.codex` language. Sketch:

**`package.json`**

```jsonc
{
  "name": "codex-lsp-vscode",
  "engines": { "vscode": "^1.75.0" },
  "activationEvents": ["onLanguage:codex"],
  "main": "./out/extension.js",
  "contributes": {
    "languages": [
      { "id": "codex", "extensions": [".codex"], "aliases": ["Codex"] }
    ]
  },
  "dependencies": { "vscode-languageclient": "^9.0.0" }
}
```

**`src/extension.ts`**

```ts
import { ExtensionContext } from "vscode";
import { LanguageClient, ServerOptions, TransportKind, LanguageClientOptions } from "vscode-languageclient/node";

let client: LanguageClient;

export function activate(_ctx: ExtensionContext) {
  const run = { command: "codex-lsp", transport: TransportKind.stdio };
  const serverOptions: ServerOptions = { run, debug: run };
  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "codex" }],
  };
  client = new LanguageClient("codex", "Codex LSP", serverOptions, clientOptions);
  client.start();
}

export function deactivate() {
  return client?.stop();
}
```

Build the extension, ensure `codex-lsp` is on `PATH`, then open any `.codex`
file.
