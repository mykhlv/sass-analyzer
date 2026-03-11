# Contributing to sass-analyzer

Thank you for your interest in contributing!

## Development Setup

1. Install [Rust 1.85+](https://rustup.rs/) and [Node.js 20+](https://nodejs.org/)
2. Clone the repository and build:

```sh
cargo build --workspace
cargo test --workspace
```

3. For the VS Code extension:

```sh
cd editors/code
npm ci
npm run build
```

## Running Tests

```sh
# Run all tests
cargo test --workspace

# Run clippy
cargo clippy --workspace -- -D warnings

# Check formatting
cargo fmt --check

# Update inline snapshots (expect-test)
UPDATE_EXPECT=1 cargo test
```

## Project Structure

- `crates/sass-parser/` — SCSS parser library (lexer, parser, syntax tree)
- `crates/sass-lsp/` — Language server (LSP)
- `crates/sass-cli/` — CLI debugging tool
- `editors/code/` — VS Code extension (TypeScript)

## Code Style

- `clippy::pedantic` is enabled workspace-wide
- `rustfmt` with `max_width = 100`
- No unnecessary comments — only where logic isn't self-evident
- Use `#[rustfmt::skip]` for tabular structures (token sets, binding power tables)

## Branching

- `main` — releases only
- `dev` — integration branch
- Feature branches from `dev`, merge into `dev`

## Changelog

Update `CHANGELOG.md` (root) under `## Unreleased` when your changes are user-visible.

## Snapshot Tests

This project uses [expect-test](https://github.com/rust-analyzer/expect-test) for inline snapshots.
To update snapshots after intentional changes:

```sh
UPDATE_EXPECT=1 cargo test
```

Review the diff carefully before committing updated snapshots.
