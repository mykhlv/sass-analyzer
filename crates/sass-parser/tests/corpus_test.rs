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

#[path = "common/tree_shape.rs"]
mod tree_shape;

use tree_shape::{Shape, describe_record_diff};

fn corpus_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test-corpus")
}

fn collect_source_files(dir: &Path) -> Vec<PathBuf> {
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
        } else if path
            .extension()
            .is_some_and(|ext| ext == "scss" || ext == "sass")
        {
            out.push(path);
        }
    }
}

struct FileResult {
    path: PathBuf,
    round_trip_ok: bool,
    error_count: usize,
    errors: Vec<String>,
    shape: Shape,
}

fn parse_corpus_dir(dir: &Path) -> Vec<FileResult> {
    let files = collect_source_files(dir);
    assert!(
        !files.is_empty(),
        "no .scss or .sass files found in {}",
        dir.display()
    );

    files
        .into_iter()
        .map(|path| {
            let source = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));

            let parse_fn = if path.extension().is_some_and(|ext| ext == "sass") {
                sass_parser::parse_sass
            } else {
                sass_parser::parse_scss
            };
            let (green, errors) = parse_fn(&source);
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
                shape: Shape::of(&tree),
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

/// One baseline-gated group of corpora, recorded in its own tracked file.
struct Gate {
    /// Human-readable name used in the header and in failure messages.
    label: &'static str,
    /// Name of the `#[test]` that runs this gate, for the regenerate hint.
    test_name: &'static str,
    /// Tracked baseline file name under `test-corpus/`.
    baseline: &'static str,
    /// Untracked sibling written instead of the baseline when a run differs, so
    /// the tracked reference is never overwritten with an unreviewed result.
    actual: &'static str,
    /// (baseline label, corpus subdirectory) pairs.
    corpora: &'static [(&'static str, &'static str)],
}

const SASS_GATE: Gate = Gate {
    label: "Indented-Sass",
    test_name: "sass_corpora_match_baseline",
    baseline: "sass-corpus-baseline.txt",
    actual: "sass-corpus-baseline.actual.txt",
    corpora: &[
        ("bulma-sass", "bulma-sass/sass"),
        ("vuetify", "vuetify/src"),
        ("quasar", "quasar/src"),
    ],
};

const SCSS_GATE: Gate = Gate {
    label: "SCSS",
    test_name: "scss_corpora_match_baseline",
    baseline: "scss-corpus-baseline.txt",
    actual: "scss-corpus-baseline.actual.txt",
    corpora: &[
        ("bootstrap", "bootstrap/scss"),
        ("foundation", "foundation/scss"),
        ("primer", "primer/src"),
        ("bulma", "bulma/sass"),
        ("angular-material", "angular-material/scss"),
    ],
};

/// Parse one corpus. Round-trip failures fail the test outright; everything else
/// is returned as baseline records, one per file:
/// `<corpus>\t<path>\t<errors>\t<rule_sets>\t<declarations>\t<blocks>\t<tree_hash>`.
fn corpus_records(name: &str, subdir: &str) -> Vec<String> {
    let dir = corpus_root().join(subdir);

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

    results
        .iter()
        .map(|r| {
            let rel = r.path.strip_prefix(&dir).unwrap_or(&r.path);
            r.shape.record(name, &rel.to_string_lossy(), r.error_count)
        })
        .collect()
}

fn record_errors(record: &str) -> usize {
    record
        .split('\t')
        .nth(2)
        .and_then(|n| n.parse().ok())
        .unwrap_or(0)
}

/// Read `# files parsed: <name> N` for one corpus from a committed baseline.
fn committed_parsed_count(committed: &str, name: &str) -> Option<usize> {
    let prefix = format!("# files parsed: {name} ");
    committed
        .lines()
        .find_map(|l| l.strip_prefix(prefix.as_str()))
        .and_then(|n| n.trim().parse().ok())
}

/// Parse every corpus of `gate` and compare the result with its tracked baseline.
///
/// Every record is compared exactly. Error counts alone are blind to the worst
/// failure class of a layout-sensitive parser: a line silently absorbed into the
/// wrong node, with no diagnostic. The shape columns turn that into a diff.
fn run_gate(gate: &Gate) {
    let mut records = Vec::new();
    let mut parsed_counts = Vec::new();
    for (name, subdir) in gate.corpora {
        let corpus = corpus_records(name, subdir);
        parsed_counts.push((*name, corpus.len()));
        records.extend(corpus);
    }
    records.sort();
    let failing = records.iter().filter(|r| record_errors(r) > 0).count();

    let mut out = format!(
        "# {} corpus baseline: one record per file.\n\
         # Checked with:\n\
         #   cargo test -p sass-parser --test corpus_test -- --ignored {}\n\
         # A run that differs writes test-corpus/{} instead of this file;\n\
         # once every change in it is reviewed as intended, copy it over this one.\n\
         # Columns: corpus, path, parse errors, RULE_SET / DECLARATION / BLOCK node\n\
         # counts, and an FNV-1a hash of the file's debug_tree. Records are compared\n\
         # exactly: a tree that changes shape with no new error (a line absorbed into\n\
         # the wrong node, a rule moved under another parent) fails like a new error.\n\
         # The per-corpus parsed counts below must not shrink either: a smaller corpus\n\
         # (partial download, retagged upstream archive, restored partial cache) can\n\
         # hide regressions behind a smaller sample.\n",
        gate.label, gate.test_name, gate.actual,
    );
    for (name, parsed) in &parsed_counts {
        let _ = writeln!(out, "# files parsed: {name} {parsed}");
    }
    let _ = writeln!(out, "# total failing files: {failing}");
    for record in &records {
        out.push_str(record);
        out.push('\n');
    }

    let test_corpus = corpus_root();
    let baseline_path = test_corpus.join(gate.baseline);
    let actual_path = test_corpus.join(gate.actual);
    let committed = std::fs::read_to_string(&baseline_path).ok();

    let shrunk_corpora: Vec<String> = parsed_counts
        .iter()
        .filter_map(|(name, parsed)| {
            let prev = committed_parsed_count(committed.as_deref()?, name)?;
            (*parsed < prev).then(|| format!("{name} ({parsed} < {prev})"))
        })
        .collect();
    let record_diff = committed.as_deref().and_then(|text| {
        let old: Vec<&str> = text.lines().filter(|l| !l.starts_with('#')).collect();
        describe_record_diff(&old, &records)
    });

    // On any difference, leave the tracked baseline untouched and write the new
    // content to the `.actual` sibling. Overwriting the tracked file on a failing
    // run would let a second run read the regressed result back as the reference
    // and pass.
    if shrunk_corpora.is_empty() && record_diff.is_none() {
        std::fs::write(&baseline_path, &out).expect("write baseline");
        let _ = std::fs::remove_file(&actual_path);
    } else {
        std::fs::write(&actual_path, &out).expect("write baseline");
    }
    eprintln!(
        "\n{} corpus baseline: {} files, {failing} failing",
        gate.label,
        records.len(),
    );

    assert!(
        shrunk_corpora.is_empty(),
        "{} corpus shrank: {shrunk_corpora:?} parsed fewer files than the committed \
         baseline. That hides real regressions behind a smaller sample (check \
         download.sh, the upstream archive, and any restored cache). Diff against \
         {}.",
        gate.label,
        actual_path.display(),
    );
    if let Some(diff) = record_diff {
        panic!(
            "{} corpus no longer matches {}:\n{diff}\
             Review `diff {} {}`. If every change is intended, copy the .actual \
             file over the baseline and commit it.",
            gate.label,
            baseline_path.display(),
            baseline_path.display(),
            actual_path.display(),
        );
    }
}

#[test]
#[ignore = "requires downloading test corpus"]
fn sass_corpora_match_baseline() {
    run_gate(&SASS_GATE);
}

#[test]
#[ignore = "requires downloading test corpus"]
fn scss_corpora_match_baseline() {
    run_gate(&SCSS_GATE);
}

#[test]
fn harness_collects_and_parses_indented_sass() {
    let dir = std::env::temp_dir().join(format!("sass_harness_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    std::fs::write(dir.join("a.sass"), ".btn\n  color: red\n").unwrap();
    std::fs::write(dir.join("b.scss"), ".btn { color: red; }\n").unwrap();

    let results = parse_corpus_dir(&dir);

    assert_eq!(results.len(), 2, "both extensions must be collected");
    for r in &results {
        assert!(
            r.round_trip_ok,
            "round-trip failed for {}",
            r.path.display()
        );
        assert_eq!(
            r.error_count,
            0,
            "{} parsed with errors — the .sass file was parsed as SCSS",
            r.path.display()
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn shape_tells_apart_trees_with_equal_node_counts() {
    let shape = |source: &str| Shape::of(&SyntaxNode::new_root(sass_parser::parse_scss(source).0));
    let top_level = shape("@mixin m {}\n.a { b: c; }\n");
    let reparented = shape("@mixin m {\n.a { b: c; }}\n");

    assert_eq!(
        (
            top_level.rule_sets,
            top_level.declarations,
            top_level.blocks
        ),
        (
            reparented.rule_sets,
            reparented.declarations,
            reparented.blocks
        ),
        "the inputs are chosen so that the node counts agree",
    );
    assert_ne!(
        top_level.tree_hash, reparented.tree_hash,
        "moving a rule into another block must change the fingerprint",
    );
}

#[test]
fn record_diff_reports_changed_missing_and_new_files() {
    let old = ["c\ta.scss\t0\t1\t1\t1\t00", "c\tb.scss\t0\t1\t1\t1\t00"];
    let new = vec![
        "c\ta.scss\t0\t1\t0\t1\t01".to_owned(),
        "c\tc.scss\t0\t0\t0\t0\t00".to_owned(),
    ];

    assert_eq!(describe_record_diff(&old, &old.map(str::to_owned)), None);
    let diff = describe_record_diff(&old, &new).expect("records differ");
    assert!(
        diff.starts_with("1 changed, 1 no longer parsed, 1 new"),
        "{diff}"
    );
    assert!(
        diff.contains("c\ta.scss: declarations 1 → 0, tree_hash 00 → 01\n"),
        "{diff}"
    );
    assert!(diff.contains("missing: c\tb.scss"), "{diff}");
    assert!(diff.contains("new: c\tc.scss"), "{diff}");
}
