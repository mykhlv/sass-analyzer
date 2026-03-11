# sass-analyzer

Fast, reliable SCSS language support for VS Code, powered by a hand-written recursive descent parser in Rust.

Built for monorepos and large codebases where existing SCSS extensions slow down or break.

## Why sass-analyzer?

- **Speed** -- parses 1.6 MB of SCSS (Angular Material) in ~25 ms. Incremental reparsing keeps edits sub-millisecond.
- **Accuracy** -- lossless concrete syntax tree preserves every byte. Error recovery isolates problems to the smallest possible region.
- **Monorepo-ready** -- import aliases, load paths, and `node_modules` resolution work out of the box.
- **Lightweight** -- single native binary, no Node.js runtime required at startup.

## Features

### Code intelligence

- **Go to definition** -- jump to variable, function, mixin, and placeholder declarations, across files and `@use`/`@forward` boundaries
- **Find references** -- locate all usages of a symbol across the workspace
- **Rename** -- workspace-wide symbol rename with preview
- **Hover** -- doc comments, parameter signatures, and variable value previews
- **Signature help** -- parameter hints for function calls and `@include`
- **Completions** -- fuzzy-scored suggestions for variables, functions, mixins, CSS properties, and `sass:` built-in modules (`sass:math`, `sass:color`, `sass:list`, `sass:map`, `sass:meta`, `sass:selector`, `sass:string`)

### Navigation

- **Document symbols** -- outline view (Ctrl+Shift+O / Cmd+Shift+O) with all declarations in the current file
- **Workspace symbols** -- search across all SCSS files (Ctrl+T / Cmd+T)
- **Document links** -- clickable `@use`, `@forward`, and `@import` paths

### Diagnostics & highlighting

- **Real-time diagnostics** -- syntax errors reported as you type, with error recovery that keeps the rest of the file valid
- **Semantic highlighting** -- distinct token colors for variables, functions, mixins, parameters, properties, and placeholder selectors

### Performance

- **Incremental reparsing** -- only the affected subtree is re-parsed on each keystroke
- **50 ms debounce** -- batches rapid edits to avoid redundant work (configurable)
- **LRU caching** -- green trees and source texts are evicted under memory pressure

## Settings

| Setting | Default | Description |
|---------|---------|-------------|
| `sass-analyzer.loadPaths` | `[]` | Additional directories for `@use`/`@forward` resolution (relative to workspace root). |
| `sass-analyzer.importAliases` | `{}` | Map of import prefixes to target directories. Arrays map one alias to multiple directories (monorepos). |
| `sass-analyzer.prependImports` | `[]` | Modules implicitly `@use`d in every file (e.g. for Vite `additionalData`). |
| `sass-analyzer.maxFileSize` | `2000000` | Maximum file size in bytes the server will parse. |
| `sass-analyzer.debounceMs` | `50` | Delay in ms before re-parsing after an edit. |
| `sass-analyzer.maxCachedTrees` | `200` | Maximum number of parse trees kept in memory. |
| `sass-analyzer.maxCachedSources` | `200` | Maximum number of source texts kept in memory. |
| `sass-analyzer.server.path` | `""` | Path to a custom `sass-lsp` binary. Leave empty for the bundled binary. |
| `sass-analyzer.trace.server` | `"off"` | Traces communication between VS Code and the server (`"off"`, `"messages"`, `"verbose"`). |

### Monorepo example

```jsonc
// .vscode/settings.json
{
  "sass-analyzer.loadPaths": ["packages/shared/styles"],
  "sass-analyzer.importAliases": {
    "@design-system": ["packages/design-system/src"],
    "@theme": "packages/theme/scss"
  },
  "sass-analyzer.prependImports": ["@design-system/tokens"]
}
```

## Architecture

The extension is a thin TypeScript client that spawns `sass-lsp` -- a Rust binary built on `tower-lsp-server`. The server produces a lossless CST via [rowan](https://github.com/rust-analyzer/rowan), following [rust-analyzer's](https://rust-analyzer.github.io/) architecture.

## Development

### Prerequisites

- Rust 1.85+ (`cargo`)
- Node.js 20+ (`npm`)

### Build from source

```sh
# Build the LSP server
cargo build -p sass-lsp

# Install extension dependencies and build
cd editors/code
npm ci
npm run build
```

### Run in VS Code

1. Open `editors/code/` in VS Code
2. Press **F5** to launch the Extension Development Host
3. Open any `.scss` file -- the extension auto-discovers the debug binary at `target/debug/sass-lsp`

## License

MIT
