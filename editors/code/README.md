# sass-analyzer

Fast SCSS language support for VS Code, powered by a hand-written recursive descent parser in Rust.

## Features

- **Diagnostics** -- real-time syntax error reporting with error recovery
- **Semantic highlighting** -- distinct token types for variables, functions, mixins, parameters, properties, and placeholder selectors
- **Go to definition** -- jump to variable, function, mixin, and placeholder declarations across files
- **Find references** -- locate all usages of a symbol in the workspace
- **Rename** -- workspace-wide symbol rename
- **Completions** -- fuzzy-scored suggestions for variables, functions, mixins, CSS properties, and `sass:` built-in modules
- **Hover** -- doc comments, parameter signatures, and variable value previews
- **Signature help** -- parameter hints for function calls and `@include`
- **Document symbols** -- outline view with all declarations in the current file
- **Workspace symbols** -- search across all SCSS files (Ctrl+T / Cmd+T)
- **Document links** -- clickable `@use`, `@forward`, and `@import` paths
- **Incremental reparsing** -- only the affected subtree is re-parsed on each edit

## Settings

| Setting | Default | Description |
|---------|---------|-------------|
| `sass-analyzer.server.path` | `""` | Override the path to the `sass-lsp` binary. Leave empty to use the bundled binary. |
| `sass-analyzer.trace.server` | `"off"` | Traces the communication between VS Code and the language server (`"off"`, `"messages"`, `"verbose"`). |
| `sass-analyzer.loadPaths` | `[]` | Additional directories to search for `@use` and `@forward` imports. Paths are relative to workspace root. Supports `${workspaceFolder}`. |
| `sass-analyzer.importAliases` | `{}` | Map of import alias prefixes to target directories. For monorepos, use arrays to map one alias to multiple directories. |
| `sass-analyzer.prependImports` | `[]` | Module paths to implicitly `@use` in every file (e.g. for Vite `additionalData`). |

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

## Architecture

The extension is a thin TypeScript client that spawns `sass-lsp` -- a Rust binary built on tower-lsp-server. The server uses a lossless CST via rowan with 50 ms debounce and incremental reparsing for sub-millisecond response on large files.

## License

MIT
