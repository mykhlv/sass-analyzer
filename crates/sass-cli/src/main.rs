use std::path::{Path, PathBuf};
use std::process;

use clap::{Parser, Subcommand};
use miette::{Diagnostic, NamedSource, Report, SourceSpan};
use sass_parser::syntax::{SyntaxNode, debug_tree};

#[derive(Parser)]
#[command(name = "sass-cli", about = "SCSS parser and linter")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Parse a single SCSS file and print its syntax tree.
    Parse {
        /// Path to the .scss file.
        file: PathBuf,
    },
    /// Check one or more SCSS files for parse errors.
    Check {
        /// Path to a .scss file or directory of .scss files.
        path: PathBuf,
    },
    /// Lex a single SCSS file and dump its token stream.
    Lex {
        /// Path to the .scss file.
        file: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();

    let exit_code = match cli.command {
        Command::Parse { file } => cmd_parse(&file),
        Command::Check { path } => cmd_check(&path),
        Command::Lex { file } => cmd_lex(&file),
    };

    process::exit(exit_code);
}

// ── parse ────────────────────────────────────────────────────────────

fn cmd_parse(path: &Path) -> i32 {
    if let Some(code) = reject_sass(path) {
        return code;
    }

    let source = match read_file(path) {
        Ok(s) => s,
        Err(code) => return code,
    };

    let (green, errors) = sass_parser::parse(&source);
    let tree = SyntaxNode::new_root(green);

    print!("{}", debug_tree(&tree));

    if errors.is_empty() {
        return 0;
    }

    let filename = path.display().to_string();
    print_diagnostics(&filename, &source, &errors);
    2
}

// ── check ────────────────────────────────────────────────────────────

fn cmd_check(path: &Path) -> i32 {
    let files = collect_scss_files(path);

    if files.is_empty() {
        eprintln!("no .scss files found in {}", path.display());
        return 1;
    }

    let mut total_errors = 0u64;
    let mut failed_files = 0u64;

    for file in &files {
        if reject_sass(file).is_some() {
            failed_files += 1;
            continue;
        }

        let Ok(source) = read_file(file) else {
            failed_files += 1;
            continue;
        };

        let (_, errors) = sass_parser::parse(&source);

        if !errors.is_empty() {
            let filename = file.display().to_string();
            print_diagnostics(&filename, &source, &errors);
            total_errors += errors.len() as u64;
            failed_files += 1;
        }
    }

    if total_errors == 0 && failed_files == 0 {
        eprintln!(
            "checked {} file{}, no errors",
            files.len(),
            if files.len() == 1 { "" } else { "s" },
        );
        0
    } else if total_errors == 0 {
        eprintln!(
            "{failed_files} file{} could not be read",
            if failed_files == 1 { "" } else { "s" },
        );
        1
    } else {
        eprintln!(
            "{total_errors} error{} in {failed_files} file{}",
            if total_errors == 1 { "" } else { "s" },
            if failed_files == 1 { "" } else { "s" },
        );
        2
    }
}

// ── lex ──────────────────────────────────────────────────────────────

fn cmd_lex(path: &Path) -> i32 {
    if let Some(code) = reject_sass(path) {
        return code;
    }

    let source = match read_file(path) {
        Ok(s) => s,
        Err(code) => return code,
    };

    let mut lexer = sass_parser::lexer::Lexer::new(&source);
    let mut has_errors = false;
    loop {
        let (kind, text) = lexer.next_token();
        if kind == sass_parser::syntax_kind::SyntaxKind::EOF {
            println!("{kind:?}");
            break;
        }
        if kind == sass_parser::syntax_kind::SyntaxKind::ERROR {
            has_errors = true;
        }
        println!("{kind:?} {text:?}");
    }

    if has_errors { 2 } else { 0 }
}

// ── diagnostics ──────────────────────────────────────────────────────

#[derive(Debug, Diagnostic)]
#[diagnostic(severity(Error))]
struct ParseError {
    #[source_code]
    src: NamedSource<String>,
    #[label("{msg}")]
    span: SourceSpan,
    msg: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "parse error")
    }
}

impl std::error::Error for ParseError {}

fn print_diagnostics(
    filename: &str,
    source: &str,
    errors: &[(String, sass_parser::text_range::TextRange)],
) {
    for (msg, range) in errors {
        let start: usize = range.start().into();
        let len: usize = range.len().into();

        let err = ParseError {
            src: NamedSource::new(filename, source.to_owned()),
            span: (start, len).into(),
            msg: msg.clone(),
        };

        eprintln!("{:?}", Report::new(err));
    }
}

// ── helpers ──────────────────────────────────────────────────────────

fn read_file(path: &Path) -> Result<String, i32> {
    match std::fs::read_to_string(path) {
        Ok(s) => Ok(s),
        Err(e) => {
            eprintln!("error: cannot read {}: {e}", path.display());
            Err(1)
        }
    }
}

fn reject_sass(path: &Path) -> Option<i32> {
    if path.extension().is_some_and(|ext| ext == "sass") {
        eprintln!(
            "error: {} uses indented Sass syntax (.sass) which is not yet supported. \
             Only SCSS (.scss) files are supported.",
            path.display(),
        );
        Some(1)
    } else {
        None
    }
}

fn collect_scss_files(path: &Path) -> Vec<PathBuf> {
    if path.is_file() {
        return vec![path.to_path_buf()];
    }

    let mut files = Vec::new();
    collect_scss_recursive(path, &mut files);
    files.sort();
    files
}

fn collect_scss_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("warning: cannot read directory {}: {e}", dir.display());
            return;
        }
    };

    for entry in entries {
        let Ok(entry) = entry else { continue };
        let path = entry.path();

        if path.is_dir() {
            // Skip hidden dirs and node_modules
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.') || name == "node_modules" {
                continue;
            }
            collect_scss_recursive(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "scss") {
            out.push(path);
        }
    }
}
