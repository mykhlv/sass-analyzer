# Changelog

All notable changes to sass-analyzer will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

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
