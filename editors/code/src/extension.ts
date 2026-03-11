import * as fs from "fs";
import * as path from "path";
import { ExtensionContext, window, workspace } from "vscode";
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

  const exeSuffix = process.platform === "win32" ? ".exe" : "";

  // Bundled binary (production VSIX)
  const bundled = path.join(context.extensionPath, "bin", `sass-lsp${exeSuffix}`);
  if (fs.existsSync(bundled)) {
    return bundled;
  }

  // Dev fallback: cargo debug binary (editors/code/../../target/debug/sass-lsp)
  const devBinary = path.join(
    context.extensionPath, "..", "..", "target", "debug", `sass-lsp${exeSuffix}`,
  );
  if (fs.existsSync(devBinary)) {
    return devBinary;
  }

  return bundled;
}

function resolveVariables(value: string, wsRoot: string): string {
  return value
    .replace(/\$\{workspaceFolder\}/g, wsRoot)
    .replace(/\$\{workspaceRoot\}/g, wsRoot);
}

export async function activate(context: ExtensionContext): Promise<void> {
  const serverPath = getServerPath(context);

  const serverOptions: ServerOptions = {
    run: { command: serverPath },
    debug: { command: serverPath },
  };

  const config = workspace.getConfiguration("sass-analyzer");
  const workspaceRoot =
    workspace.workspaceFolders?.[0]?.uri.fsPath ?? "";

  const rawLoadPaths = config.get<string[]>("loadPaths", []);
  const rawAliases = config.get<Record<string, string | string[]>>(
    "importAliases",
    {},
  );
  const rawPrependImports = config.get<string[]>("prependImports", []);

  const loadPaths = rawLoadPaths.map((p) =>
    resolveVariables(p, workspaceRoot),
  );
  const importAliases: Record<string, string | string[]> = {};
  for (const [prefix, targets] of Object.entries(rawAliases)) {
    if (Array.isArray(targets)) {
      importAliases[prefix] = targets.map((t) =>
        resolveVariables(t, workspaceRoot),
      );
    } else {
      importAliases[prefix] = resolveVariables(targets, workspaceRoot);
    }
  }
  const prependImports = rawPrependImports.map((p) =>
    resolveVariables(p, workspaceRoot),
  );

  const outputChannel = window.createOutputChannel("sass-analyzer", "log");

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ language: "scss" }],
    connectionOptions: {
      maxRestartCount: 4,
    },
    initializationOptions: {
      loadPaths,
      importAliases,
      prependImports,
    },
    outputChannel,
    traceOutputChannel: outputChannel,
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
