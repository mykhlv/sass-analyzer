# Changelog

All notable changes to sass-analyzer will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

## 0.3.0 — 2026-03-15

### Added
- Indented Sass syntax (`.sass`) support: parse `.sass` files using whitespace-significant syntax (no braces/semicolons). All LSP features (hover, completions, go-to-definition, rename, references, diagnostics, etc.) work with `.sass` files. Includes CLI support (`sass-cli parse/check`) and VS Code extension activation for the `sass` language
- Open VSX Registry publishing in release workflow (available in Cursor, Windsurf, VSCodium, Gitpod)

### Changed
- MSRV updated from 1.85 to 1.94

### Fixed
- Parser: sass-spec compatibility improved from 99.13% to 99.78% (95 → 24 false negatives). Fixes include: trailing commas in selectors, `@elseif` (deprecated no-space form), `@extend` with comma-separated selectors, `@at-root` query with quoted strings, keyframe flexibility (interpolated selectors, variable declarations, nested at-rules, anonymous names), interpolated at-rule names, CSS `@function` declarations, `@import` with media query commas, calc with interpolation/variables/space-separated values, `alpha()`/`progid:`/`expression()` special functions, `#{}` in ID selectors and placeholders, `-#{}` in property names and selectors, `!important` in mixin arguments, `@each` with space-separated lists
- README inaccuracies corrected

## 0.2.0 — 2026-03-14

### Added
- SassDoc support: `/// @param`, `/// @return`, `/// @example`, `/// @deprecated`, `/// @type`, `/// @see`, `/// @output`, `/// @content`, `/// @throw` annotations are parsed and rendered as structured markdown in hover, completions, and signature help. Parameter descriptions appear inline in signature help popups
- Call hierarchy: navigate incoming/outgoing calls for functions and mixins — "who calls this?" and "what does this call?" (right-click → Show Call Hierarchy). Supports same-file and cross-file resolution, groups callers by enclosing function/mixin
- Inlay hints: parameter name hints for positional arguments in function calls and `@include` directives (e.g., `add(⁣$a:⁣ 1px, ⁣$b:⁣ 2px)`). Skips keyword arguments, single-parameter calls, and rest parameters
- CSS value completions: after typing `display: `, offers `flex`, `grid`, `block`, `none`, etc. Covers ~80 CSS properties with keyword values plus global keywords (`inherit`, `initial`, `unset`, `revert`, `revert-layer`). Sass variables and functions also appear in value position. AST-based context refinement correctly handles map entries and multi-line values
- Code actions (quick fixes): auto-import `@use` for undefined variables, functions, and mixins — inserts the `@use` statement and qualifies the reference with the namespace. Remove unused `@use` statements
- Code actions (refactoring): extract selection to variable (`$new-variable`), extract declarations to mixin (`@mixin new-mixin`)
- Semantic diagnostics: wrong argument count (ERROR) for functions and mixins, undefined variable/function/mixin warnings (WARNING) with false-positive suppression for `@import` files and CSS global functions
- Diagnostic cascade: when a dependency file changes, diagnostics are re-published for all open files that import it
- Selection ranges: smart expand/shrink selection (Shift+Alt+→/←) that follows the AST structure — token → node → parent → root
- Document highlights: highlight all occurrences of the symbol under the cursor (variables, functions, mixins, placeholders) with read/write distinction
- Folding ranges: collapsible regions for rule blocks, at-rules, multi-line comments, consecutive `//` comment groups, and `// #region` / `// #endregion` markers
- File watcher support: non-open SCSS/Sass files modified, created, or deleted on disk are now detected and re-indexed automatically
- Check Workspace command: run diagnostics on all SCSS files in the workspace (Command Palette → "sass-analyzer: Check Workspace"), with progress reporting
- Go to definition on `@use`/`@forward`/`@import` paths: Cmd+Click on the import string navigates directly to the target file
- Parser: value-and-block syntax (`margin: 10px { top: 20px; }`) — nested property with both a value and sub-declarations
- `rect()` CSS function recognized as valid (no false "undefined function" diagnostic)

### Removed
- Color provider (decorators and picker) — removed to avoid duplicate color squares with VS Code's built-in CSS color support

## 0.1.1 — 2026-03-12

### Added
- Cross-platform CI (Ubuntu, Windows, macOS) with cargo-deny license/vulnerability auditing
- CONTRIBUTING.md with development setup and coding guidelines
- Crate-level documentation with quick start example
- Lexer error recovery tests and non-ASCII / UTF-16 LSP tests
- `parse_file` example binary for quick syntax tree inspection
- GitHub issue templates (bug report, feature request)
- Dependabot for Cargo, npm, and GitHub Actions dependencies
- `rust-toolchain.toml` pinning stable toolchain
- Linux ARM64 (`aarch64-unknown-linux-gnu`) release target
- `#![warn(missing_docs)]` with doc comments on all public API items in `sass-parser`
- Expanded extension README with feature overview, monorepo example, and tuning settings

### Changed
- Runtime constants (`maxFileSize`, `debounceMs`, `maxCachedTrees`, `maxCachedSources`) are now configurable via `initializationOptions` and `workspace/didChangeConfiguration`
- LRU eviction for source text in module graph to cap memory usage
- Completion handler avoids cloning full document text
- `@use` path completion runs on a blocking thread to avoid stalling the LSP
- Symbols shared via `Arc<FileSymbols>` instead of deep cloning per request
- Dependency indexing capped at 10,000 files per workspace with TOCTOU guard

### Fixed
- Correct UTF-16 column offsets for non-ASCII content in diagnostics, go-to-definition, hover, references, rename, and document symbols
- Windows: extension now finds the bundled `sass-lsp.exe` binary
- `path_to_uri` no longer panics on paths with special characters
- Silent failure when LSP worker channel drops (now logs error)
- Path traversal protection in module resolver (reject `..` escaping workspace roots)
- Files exceeding size limit now logged instead of silently skipped
- Extension now forwards `didChangeConfiguration` to the server (settings changes take effect without restart)
- Extension checks server binary exists before starting, shows actionable error instead of cryptic crash
- Extension awaits `client.start()` so startup errors surface properly
- Defensive string slicing in `@use`/`@forward` path extraction prevents panic on malformed tokens
- `byte_to_lsp_pos` clamps out-of-range offsets to avoid panic on stale parse trees
- `didChangeConfiguration` now rebuilds module resolver, `loadPaths`, `importAliases`, and `prependImports` (previously only numeric settings updated live)
- `merge_errors` clamps negative shifted offsets to zero instead of wrapping to corrupt ranges
- Lexer now handles scientific notation in numbers (`1e3`, `2.5e-2`, `1E+3`)
- Signature help parameter offsets now use UTF-16 code units (fixes highlight for non-ASCII parameter names)
- `path_to_uri` fallback no longer produces `file:////path` (4 slashes) on Unix
- Extension now declares `extensionKind: ["workspace"]` for SSH/WSL/Container remote dev

## 0.1.0 — 2026-03-08

Initial release.

- Real-time diagnostics with error recovery
- Semantic highlighting for variables, functions, mixins, parameters, properties, placeholders
- Go to definition for variables, functions, mixins, placeholders
- Find all references
- Rename symbol (workspace-wide)
- Completions with fuzzy scoring (variables, functions, mixins, CSS properties, built-in modules)
- Hover with doc comments and value previews
- Signature help for functions and mixins
- Document symbols (Outline view)
- Workspace symbols (Ctrl+T)
- Document links for `@use`, `@forward`, `@import`
- Multi-file module graph with `@use` / `@forward` resolution
- Incremental text sync and incremental reparsing
- Import alias and load path configuration for monorepos
- Platform-specific binaries: Linux x64, Windows x64, macOS x64, macOS ARM64
