import * as fs from "fs";
import * as path from "path";
import { ExtensionContext, window, workspace } from "vscode";
import {
  DidChangeConfigurationNotification,
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

function buildSettings(
  config: ReturnType<typeof workspace.getConfiguration>,
  primaryRoot: string,
): Record<string, unknown> {
  const rawLoadPaths = config.get<string[]>("loadPaths", []);
  const rawAliases = config.get<Record<string, string | string[]>>(
    "importAliases",
    {},
  );
  const rawPrependImports = config.get<string[]>("prependImports", []);

  const importAliases: Record<string, string | string[]> = {};
  for (const [prefix, targets] of Object.entries(rawAliases)) {
    importAliases[prefix] = Array.isArray(targets)
      ? targets.map((t) => resolveVariables(t, primaryRoot))
      : resolveVariables(targets, primaryRoot);
  }

  return {
    loadPaths: rawLoadPaths.map((p) => resolveVariables(p, primaryRoot)),
    importAliases,
    prependImports: rawPrependImports.map((p) =>
      resolveVariables(p, primaryRoot),
    ),
    reportUnresolvedImports: config.get<boolean>("reportUnresolvedImports"),
    maxFileSize: config.get<number>("maxFileSize"),
    debounceMs: config.get<number>("debounceMs"),
    maxCachedTrees: config.get<number>("maxCachedTrees"),
    maxCachedSources: config.get<number>("maxCachedSources"),
  };
}

export async function activate(context: ExtensionContext): Promise<void> {
  const serverPath = getServerPath(context);

  const serverOptions: ServerOptions = {
    run: { command: serverPath },
    debug: { command: serverPath },
  };

  const config = workspace.getConfiguration("sass-analyzer");
  const workspaceRoots =
    workspace.workspaceFolders?.map((f) => f.uri.fsPath) ?? [];
  const primaryRoot = workspaceRoots[0] ?? "";

  const outputChannel = window.createOutputChannel("sass-analyzer", "log");

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ language: "scss" }, { language: "sass" }],
    connectionOptions: {
      maxRestartCount: 4,
    },
    initializationOptions: buildSettings(config, primaryRoot),
    outputChannel,
    traceOutputChannel: outputChannel,
  };

  client = new LanguageClient(
    "sass-analyzer",
    "sass-analyzer",
    serverOptions,
    clientOptions,
  );

  if (!fs.existsSync(serverPath)) {
    window.showErrorMessage(
      `sass-analyzer: server binary not found at ${serverPath}. ` +
      `Run \`cargo build -p sass-lsp\` or set sass-analyzer.server.path.`,
    );
    return;
  }

  await client.start();

  context.subscriptions.push(
    workspace.onDidChangeConfiguration(async (event) => {
      if (!event.affectsConfiguration("sass-analyzer") || !client) {
        return;
      }
      const updated = workspace.getConfiguration("sass-analyzer");
      await client.sendNotification(DidChangeConfigurationNotification.type, {
        settings: { "sass-analyzer": buildSettings(updated, primaryRoot) },
      });
    }),
  );
}

export async function deactivate(): Promise<void> {
  if (client) {
    await client.stop();
  }
}
