use std::path::{Path, PathBuf};
use std::process;
use std::sync::Arc;

use clap::{Parser, Subcommand};
use miette::{
    Diagnostic, MietteError, MietteSpanContents, NamedSource, Report, SourceCode, SourceSpan,
    SpanContents,
};
use sass_parser::syntax::{SyntaxNode, debug_tree};

#[derive(Parser)]
#[command(name = "sass-cli", about = "SCSS parser and linter", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Parse a single SCSS/Sass file and print its syntax tree.
    Parse {
        /// Path to the .scss or .sass file.
        file: PathBuf,
    },
    /// Check one or more SCSS/Sass files for parse errors.
    Check {
        /// Path to a .scss/.sass file or directory.
        path: PathBuf,
    },
    /// Lex a single SCSS/Sass file and dump its token stream.
    Lex {
        /// Path to the .scss or .sass file.
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
    let source = match read_file(path) {
        Ok(s) => s,
        Err(code) => return code,
    };

    let (green, errors) = if is_sass(path) {
        sass_parser::parse_sass(&source)
    } else {
        sass_parser::parse_scss(&source)
    };
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
    let files = collect_sass_files(path);

    if files.is_empty() {
        eprintln!("no .scss/.sass files found in {}", path.display());
        return 1;
    }

    let mut total_errors = 0u64;
    let mut failed_files = 0u64;

    for file in &files {
        let Ok(source) = read_file(file) else {
            failed_files += 1;
            continue;
        };

        let (_, errors) = if is_sass(file) {
            sass_parser::parse_sass(&source)
        } else {
            sass_parser::parse_scss(&source)
        };

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
    let source = match read_file(path) {
        Ok(s) => s,
        Err(code) => return code,
    };

    // The token stream the parser starts from. For indented Sass that is the SCSS
    // tokens with layout-dependent comment extent; where statements end and blocks
    // open is decided by the parser, so `parse` shows those.
    if is_sass(path) {
        dump_tokens(sass_parser::lexer::IndentedLexer::crossing_blank_lines(
            &source,
        ))
    } else {
        dump_tokens(sass_parser::lexer::Lexer::new(&source))
    }
}

fn dump_tokens<'src>(mut lexer: impl sass_parser::lexer::TokenSource<'src>) -> i32 {
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
struct ParseError<S: SourceCode + std::fmt::Debug + 'static> {
    #[source_code]
    src: NamedSource<S>,
    #[label("{msg}")]
    span: SourceSpan,
    msg: String,
}

impl<S: SourceCode + std::fmt::Debug + 'static> std::fmt::Display for ParseError<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "parse error")
    }
}

impl<S: SourceCode + std::fmt::Debug + 'static> std::error::Error for ParseError<S> {}

/// A file's text shared by all of its diagnostics, with the start of each line.
///
/// miette reads a span's context by scanning the source from its start, and each
/// diagnostic used to own a copy of the source, so a file with many errors took
/// time quadratic in its size. This hands miette only the lines around the span,
/// found by binary search, so each diagnostic costs the size of its context.
#[derive(Debug, Clone)]
struct SharedSource {
    text: Arc<str>,
    /// Byte offset of each line's start. Lines end at `\r\n`, `\n` or a lone `\r`,
    /// as miette counts them.
    line_starts: Arc<[usize]>,
}

impl SharedSource {
    fn new(text: &str) -> Self {
        let bytes = text.as_bytes();
        let mut line_starts = vec![0];
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'\r' if bytes.get(i + 1) == Some(&b'\n') => i += 1,
                b'\r' | b'\n' => {}
                _ => {
                    i += 1;
                    continue;
                }
            }
            i += 1;
            line_starts.push(i);
        }
        Self {
            text: text.into(),
            line_starts: line_starts.into(),
        }
    }

    fn line_of(&self, offset: usize) -> usize {
        self.line_starts.partition_point(|&start| start <= offset) - 1
    }
}

impl SourceCode for SharedSource {
    fn read_span<'a>(
        &'a self,
        span: &SourceSpan,
        context_lines_before: usize,
        context_lines_after: usize,
    ) -> Result<Box<dyn SpanContents<'a> + 'a>, MietteError> {
        let start = span.offset();
        let end = start + span.len();
        if end > self.text.len() {
            return Err(MietteError::OutOfBounds);
        }
        // One line of margin past what miette reads, so that its scan of the
        // window stops where its scan of the whole text would.
        let first = self.line_of(start).saturating_sub(context_lines_before);
        let last = self.line_of(end) + context_lines_after + 2;
        let window_start = self.line_starts[first];
        let window_end = self
            .line_starts
            .get(last)
            .copied()
            .unwrap_or(self.text.len());
        let window = &self.text[window_start..window_end];
        let inner = window.read_span(
            &SourceSpan::new((start - window_start).into(), span.len()),
            context_lines_before,
            context_lines_after,
        )?;
        Ok(Box::new(MietteSpanContents::new(
            inner.data(),
            SourceSpan::new(
                (inner.span().offset() + window_start).into(),
                inner.span().len(),
            ),
            inner.line() + first,
            inner.column(),
            inner.line_count(),
        )))
    }
}

fn print_diagnostics(
    filename: &str,
    source: &str,
    errors: &[(String, sass_parser::text_range::TextRange)],
) {
    use std::io::Write as _;

    // stderr is unbuffered, and a report is written in many small pieces.
    let mut out = std::io::BufWriter::new(std::io::stderr().lock());
    for report in diagnostic_reports(filename, source, errors) {
        let _ = writeln!(out, "{report:?}");
    }
}

/// One report per parse error, all reading the one shared copy of `source`.
fn diagnostic_reports(
    filename: &str,
    source: &str,
    errors: &[(String, sass_parser::text_range::TextRange)],
) -> impl Iterator<Item = Report> {
    let src = NamedSource::new(filename, SharedSource::new(source));
    errors.iter().map(move |(msg, range)| {
        Report::new(ParseError {
            src: src.clone(),
            span: (usize::from(range.start()), usize::from(range.len())).into(),
            msg: msg.clone(),
        })
    })
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

fn is_sass(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "sass")
}

fn collect_sass_files(path: &Path) -> Vec<PathBuf> {
    if path.is_file() {
        return vec![path.to_path_buf()];
    }

    let mut files = Vec::new();
    collect_sass_recursive(path, &mut files);
    files.sort();
    files
}

fn collect_sass_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
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
            collect_sass_recursive(&path, out);
        } else if path
            .extension()
            .is_some_and(|ext| ext == "scss" || ext == "sass")
        {
            out.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use miette::{GraphicalReportHandler, GraphicalTheme};

    fn render(report: &dyn Diagnostic) -> String {
        let mut out = String::new();
        GraphicalReportHandler::new_themed(GraphicalTheme::unicode_nocolor())
            .render_report(&mut out, report)
            .unwrap();
        out
    }

    /// Every diagnostic renders as it does from the whole text.
    #[test]
    fn shared_source_renders_as_the_whole_text() {
        let inputs = [
            ".a { $: 1; }\n",
            ".a { b: c; }\n.x { $: 1; }\n.y { $: 2; }\n.a { b: c; }\n",
            ".a { b: c; }\r\n.x { $: 1; }\r\n\r\n.a { b: c; }\r\n",
            ".a { b: c; }\r.x { $: 1; }\r.a { b: c; }\r",
            "\n\n\n.x { $: 1; }",
            ")",
            ".a {\n  b: c;\n",
            "@include",
            "a { b: (1 + ; }\n\n\n\n\n\n\n.c { d: ) }\n",
        ];
        for input in inputs {
            let (_, errors) = sass_parser::parse_scss(input);
            assert!(!errors.is_empty(), "{input:?} has no errors");
            let whole = NamedSource::new("t.scss", input.to_owned());
            for (report, (msg, range)) in diagnostic_reports("t.scss", input, &errors).zip(&errors)
            {
                let expected = ParseError {
                    src: whole.clone(),
                    span: (usize::from(range.start()), usize::from(range.len())).into(),
                    msg: msg.clone(),
                };
                assert_eq!(render(report.as_ref()), render(&expected), "{input:?}");
            }
        }
    }

    /// Many errors in one file render; each costs the size of its context.
    #[test]
    fn many_errors_render() {
        let source = ".a { b: c; }\n".repeat(2000) + &".x { $: 1; }\n".repeat(2000);
        let (_, errors) = sass_parser::parse_scss(&source);
        assert_eq!(errors.len(), 2000);
        let last = diagnostic_reports("t.scss", &source, &errors)
            .map(|report| render(report.as_ref()))
            .last()
            .unwrap();
        assert!(last.contains("4000 │ .x { $: 1; }"), "{last}");
    }
}
