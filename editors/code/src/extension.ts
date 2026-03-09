import * as fs from "fs";
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

  // Bundled binary (production VSIX)
  const bundled = path.join(context.extensionPath, "bin", "sass-lsp");
  if (fs.existsSync(bundled)) {
    return bundled;
  }

  // Dev fallback: cargo debug binary (editors/code/../../target/debug/sass-lsp)
  const devBinary = path.join(context.extensionPath, "..", "..", "target", "debug", "sass-lsp");
  if (fs.existsSync(devBinary)) {
    return devBinary;
  }

  return bundled;
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
