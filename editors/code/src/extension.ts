import * as path from "path";
import { ExtensionContext, workspace } from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

function getServerPath(context: ExtensionContext): string {
  const config = workspace.getConfiguration("sass-analyzer");
  const customPath = config.get<string>("server.path");
  if (customPath) {
    return customPath;
  }
  return path.join(context.extensionPath, "bin", "sass-lsp");
}

export async function activate(context: ExtensionContext): Promise<void> {
  const serverPath = getServerPath(context);

  const serverOptions: ServerOptions = {
    run: { command: serverPath },
    debug: { command: serverPath },
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ language: "scss" }],
    connectionOptions: {
      maxRestartCount: 4,
    },
  };

  client = new LanguageClient(
    "sass-analyzer",
    "sass-analyzer",
    serverOptions,
    clientOptions,
  );

  client.start();
}

export async function deactivate(): Promise<void> {
  if (client) {
    await client.stop();
  }
}
