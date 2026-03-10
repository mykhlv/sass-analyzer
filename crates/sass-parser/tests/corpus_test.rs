//! Real-world corpus validation tests.
//!
//! These tests require downloading the test corpus first:
//!   cd test-corpus && bash download.sh
//!
//! Run with:
//!   `cargo test --test corpus_test -- --ignored --nocapture`

use std::fmt::Write;
use std::path::{Path, PathBuf};

use sass_parser::syntax::SyntaxNode;

fn corpus_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test-corpus")
}

fn collect_scss_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_recursive(dir, &mut files);
    files.sort();
    files
}

fn collect_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries {
        let Ok(entry) = entry else { continue };
        let path = entry.path();

        if path.is_dir() {
            collect_recursive(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "scss") {
            out.push(path);
        }
    }
}

struct FileResult {
    path: PathBuf,
    round_trip_ok: bool,
    error_count: usize,
    errors: Vec<String>,
}

fn parse_corpus_dir(dir: &Path) -> Vec<FileResult> {
    let files = collect_scss_files(dir);
    assert!(
        !files.is_empty(),
        "no .scss files found in {}",
        dir.display()
    );

    files
        .into_iter()
        .map(|path| {
            let source = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));

            let (green, errors) = sass_parser::parse(&source);
            let tree = SyntaxNode::new_root(green);

            let round_trip_ok = tree.text().to_string() == source;

            let error_strings: Vec<String> = errors
                .iter()
                .take(5)
                .map(|(msg, range)| format!("  {range:?}: {msg}"))
                .collect();

            FileResult {
                path,
                round_trip_ok,
                error_count: errors.len(),
                errors: error_strings,
            }
        })
        .collect()
}

fn print_corpus_report(corpus_name: &str, results: &[FileResult], corpus_dir: &Path) {
    let total = results.len();
    let round_trip_failures: Vec<_> = results.iter().filter(|r| !r.round_trip_ok).collect();
    let files_with_errors: Vec<_> = results.iter().filter(|r| r.error_count > 0).collect();
    let total_errors: usize = results.iter().map(|r| r.error_count).sum();

    let mut report = String::new();
    writeln!(report, "\n=== {corpus_name} corpus report ===").unwrap();
    writeln!(report, "  files parsed: {total}").unwrap();
    writeln!(
        report,
        "  round-trip OK: {}",
        total - round_trip_failures.len()
    )
    .unwrap();
    writeln!(report, "  files with errors: {}", files_with_errors.len()).unwrap();
    writeln!(report, "  total errors: {total_errors}").unwrap();

    if !round_trip_failures.is_empty() {
        writeln!(report, "\n  ROUND-TRIP FAILURES:").unwrap();
        for r in &round_trip_failures {
            let rel = r.path.strip_prefix(corpus_dir).unwrap_or(&r.path);
            writeln!(report, "    {}", rel.display()).unwrap();
        }
    }

    if !files_with_errors.is_empty() {
        writeln!(report, "\n  FILES WITH PARSE ERRORS:").unwrap();
        for r in &files_with_errors {
            let rel = r.path.strip_prefix(corpus_dir).unwrap_or(&r.path);
            writeln!(report, "    {} ({} errors)", rel.display(), r.error_count).unwrap();
            for e in &r.errors {
                writeln!(report, "      {e}").unwrap();
            }
            if r.error_count > 5 {
                writeln!(report, "      ... and {} more", r.error_count - 5).unwrap();
            }
        }
    }

    eprint!("{report}");
}

#[test]
#[ignore = "requires downloading test corpus"]
fn bootstrap_scss_zero_panics_and_round_trip() {
    let corpus = corpus_root();
    let bootstrap_dir = corpus.join("bootstrap/scss");

    assert!(
        bootstrap_dir.exists(),
        "Bootstrap corpus not found at {}. Run: cd test-corpus && bash download.sh",
        bootstrap_dir.display()
    );

    let results = parse_corpus_dir(&bootstrap_dir);
    print_corpus_report("Bootstrap", &results, &bootstrap_dir);

    // Hard assertion: lossless round-trip must hold for ALL files.
    let rt_failures: Vec<_> = results
        .iter()
        .filter(|r| !r.round_trip_ok)
        .map(|r| {
            r.path
                .strip_prefix(&bootstrap_dir)
                .unwrap_or(&r.path)
                .display()
                .to_string()
        })
        .collect();
    assert!(
        rt_failures.is_empty(),
        "round-trip failed for {} files: {:?}",
        rt_failures.len(),
        rt_failures,
    );

    // Parse errors are documented (printed above), not asserted.
    // Task 5.14 says "document parse failures", not "zero errors".
    let files_with_errors = results.iter().filter(|r| r.error_count > 0).count();
    if files_with_errors > 0 {
        let total_errors: usize = results.iter().map(|r| r.error_count).sum();
        eprintln!(
            "\nNOTE: {files_with_errors} files have parse errors ({total_errors} total). \
             This is documented, not a test failure."
        );
    }
}

/// Run a standard corpus test: parse all .scss files, assert round-trip, report errors.
fn run_corpus_test(name: &str, subdir: &str) {
    let corpus = corpus_root();
    let dir = corpus.join(subdir);

    assert!(
        dir.exists(),
        "{name} corpus not found at {}. Run: cd test-corpus && bash download.sh",
        dir.display()
    );

    let results = parse_corpus_dir(&dir);
    print_corpus_report(name, &results, &dir);

    let rt_failures: Vec<_> = results
        .iter()
        .filter(|r| !r.round_trip_ok)
        .map(|r| {
            r.path
                .strip_prefix(&dir)
                .unwrap_or(&r.path)
                .display()
                .to_string()
        })
        .collect();
    assert!(
        rt_failures.is_empty(),
        "round-trip failed for {} files: {:?}",
        rt_failures.len(),
        rt_failures,
    );
}

#[test]
#[ignore = "requires downloading test corpus"]
fn foundation_scss_zero_panics_and_round_trip() {
    run_corpus_test("Foundation", "foundation/scss");
}

#[test]
#[ignore = "requires downloading test corpus"]
fn primer_scss_zero_panics_and_round_trip() {
    run_corpus_test("Primer", "primer/src");
}

#[test]
#[ignore = "requires downloading test corpus"]
fn bulma_scss_zero_panics_and_round_trip() {
    run_corpus_test("Bulma", "bulma/sass");
}

#[test]
#[ignore = "requires downloading test corpus"]
fn angular_material_scss_zero_panics_and_round_trip() {
    run_corpus_test("Angular Material", "angular-material/scss");
}
