# Changelog

All notable changes to sass-analyzer will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

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

### Changed
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
