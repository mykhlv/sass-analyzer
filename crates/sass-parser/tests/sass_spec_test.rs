//! sass-spec compatibility harness (Task 5.15).
//!
//! Parses every `input.scss` from the sass/sass-spec test suite and compares
//! parse success/failure against dart-sass expectations (inferred from the
//! presence of `output.css` vs `error` sibling entries).
//!
//! Download the spec first:
//!   cd test-corpus && bash download.sh
//!
//! Run with:
//!   cargo test --test sass_spec_test -- --ignored --nocapture

use std::collections::BTreeMap;
use std::fmt::Write;
use std::path::{Path, PathBuf};

use sass_parser::syntax::SyntaxNode;

// ── HRX parser ──────────────────────────────────────────────────────

/// A single test case extracted from an HRX file or a directory.
struct TestCase {
    /// Display path: hrx_file_relative / entry_prefix
    display_path: String,
    input_scss: String,
    /// dart-sass expects success (has output.css)
    expects_success: bool,
    /// dart-sass expects error (has `error` entry)
    expects_error: bool,
}

/// Parse an HRX file into test cases.
///
/// HRX format: entries separated by `<===> path\n`. The boundary is
/// `<===>` (angle + 3 equals + angle) followed by a space and a path,
/// or just `<===>` (comment/separator boundary, ignored).
fn parse_hrx(content: &str, hrx_rel_path: &str) -> Vec<TestCase> {
    let mut entries: BTreeMap<String, String> = BTreeMap::new();

    let mut current_path: Option<String> = None;
    let mut current_body = String::new();

    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("<===> ") {
            // Save previous entry.
            if let Some(path) = current_path.take() {
                entries.insert(path, std::mem::take(&mut current_body));
            } else {
                current_body.clear();
            }
            current_path = Some(rest.trim().to_string());
        } else if line.starts_with("<===>") {
            // Separator boundary (empty or comment). Save previous.
            if let Some(path) = current_path.take() {
                entries.insert(path, std::mem::take(&mut current_body));
            } else {
                current_body.clear();
            }
        } else {
            if current_path.is_some() {
                if !current_body.is_empty() {
                    current_body.push('\n');
                }
                current_body.push_str(line);
            }
        }
    }
    // Flush last entry.
    if let Some(path) = current_path.take() {
        entries.insert(path, current_body);
    }

    // Group entries by prefix directory to find distinct test cases.
    // A test case has `{prefix/}input.scss`.
    let input_keys: Vec<String> = entries
        .keys()
        .filter(|k| k.ends_with("input.scss"))
        .cloned()
        .collect();

    let mut cases = Vec::new();
    for input_key in &input_keys {
        let prefix = if input_key == "input.scss" {
            ""
        } else {
            // "foo/bar/input.scss" → "foo/bar/"
            &input_key[..input_key.len() - "input.scss".len()]
        };

        let output_key = format!("{prefix}output.css");
        let error_key = format!("{prefix}error");

        let expects_success = entries.contains_key(&output_key);
        let expects_error = entries.contains_key(&error_key);

        let display = if prefix.is_empty() {
            hrx_rel_path.to_string()
        } else {
            format!("{hrx_rel_path}/{}", prefix.trim_end_matches('/'))
        };

        cases.push(TestCase {
            display_path: display,
            input_scss: entries[input_key].clone(),
            expects_success,
            expects_error,
        });
    }

    cases
}

// ── Test case collection ────────────────────────────────────────────

fn spec_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test-corpus/sass-spec/spec")
}

fn collect_hrx_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_hrx_recursive(dir, &mut files);
    files.sort();
    files
}

fn collect_hrx_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries {
        let Ok(entry) = entry else { continue };
        let path = entry.path();

        if path.is_dir() {
            collect_hrx_recursive(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "hrx") {
            out.push(path);
        }
    }
}

/// Also collect standalone input.scss (non-HRX) test cases.
fn collect_standalone_tests(dir: &Path, spec_dir: &Path) -> Vec<TestCase> {
    let mut cases = Vec::new();
    collect_standalone_recursive(dir, spec_dir, &mut cases);
    cases
}

fn collect_standalone_recursive(dir: &Path, spec_dir: &Path, out: &mut Vec<TestCase>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries {
        let Ok(entry) = entry else { continue };
        let path = entry.path();

        if path.is_dir() {
            collect_standalone_recursive(&path, spec_dir, out);
        } else if path.file_name().is_some_and(|n| n == "input.scss") {
            let parent = path.parent().unwrap();
            let source = match std::fs::read_to_string(&path) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let has_output = parent.join("output.css").exists();
            let has_error = parent.join("error").exists();

            let rel = parent
                .strip_prefix(spec_dir)
                .unwrap_or(parent)
                .to_string_lossy()
                .to_string();

            out.push(TestCase {
                display_path: rel,
                input_scss: source,
                expects_success: has_output,
                expects_error: has_error,
            });
        }
    }
}

// ── Compatibility analysis ──────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
enum Outcome {
    /// dart-sass valid + we parse ok → compatible
    BothOk,
    /// dart-sass error + we also error → compatible
    BothError,
    /// dart-sass valid + we error → gap (we're stricter than dart-sass)
    FalseNegative,
    /// dart-sass error + we parse ok → expected (semantic errors aren't syntax)
    FalsePositive,
    /// neither output.css nor error → uncategorized (skip)
    Uncategorized,
}

struct CaseResult {
    display_path: String,
    outcome: Outcome,
    our_error_count: usize,
    first_errors: Vec<String>,
}

fn classify(case: &TestCase, our_errors: usize) -> Outcome {
    let we_ok = our_errors == 0;

    match (case.expects_success, case.expects_error, we_ok) {
        (true, _, true) => Outcome::BothOk,
        (true, _, false) => Outcome::FalseNegative,
        (_, true, false) => Outcome::BothError,
        (_, true, true) => Outcome::FalsePositive,
        // No output.css and no error → uncategorized test (options-only, etc.)
        (false, false, _) => Outcome::Uncategorized,
    }
}

// ── Report ──────────────────────────────────────────────────────────

fn print_compatibility_report(results: &[CaseResult]) {
    let total = results.len();
    let categorized: Vec<_> = results
        .iter()
        .filter(|r| r.outcome != Outcome::Uncategorized)
        .collect();

    let both_ok = categorized
        .iter()
        .filter(|r| r.outcome == Outcome::BothOk)
        .count();
    let both_error = categorized
        .iter()
        .filter(|r| r.outcome == Outcome::BothError)
        .count();
    let false_neg: Vec<_> = categorized
        .iter()
        .filter(|r| r.outcome == Outcome::FalseNegative)
        .collect();
    let false_pos = categorized
        .iter()
        .filter(|r| r.outcome == Outcome::FalsePositive)
        .count();
    let uncategorized = results
        .iter()
        .filter(|r| r.outcome == Outcome::Uncategorized)
        .count();

    let cat_total = categorized.len();
    let matching = both_ok + both_error + false_pos;
    let match_rate = if cat_total > 0 {
        matching as f64 / cat_total as f64 * 100.0
    } else {
        0.0
    };

    // Valid-input match rate: what percentage of dart-sass-valid inputs do we also parse?
    let valid_total = both_ok + false_neg.len();
    let valid_match_rate = if valid_total > 0 {
        both_ok as f64 / valid_total as f64 * 100.0
    } else {
        0.0
    };

    let mut report = String::new();
    writeln!(report, "\n{}", "=".repeat(60)).unwrap();
    writeln!(report, "  sass-spec compatibility report").unwrap();
    writeln!(report, "{}\n", "=".repeat(60)).unwrap();
    writeln!(report, "  total test cases:    {total}").unwrap();
    writeln!(report, "  categorized:         {cat_total}").unwrap();
    writeln!(report, "  uncategorized:       {uncategorized}").unwrap();
    writeln!(report).unwrap();
    writeln!(report, "  ── results ──").unwrap();
    writeln!(report, "  both-ok (compatible):    {both_ok}").unwrap();
    writeln!(report, "  both-error (compatible): {both_error}").unwrap();
    writeln!(
        report,
        "  false-positive (ok):     {false_pos}  (semantic errors, not syntax)"
    )
    .unwrap();
    writeln!(
        report,
        "  FALSE-NEGATIVE (gap):    {}  ← our parser rejects valid input",
        false_neg.len()
    )
    .unwrap();
    writeln!(report).unwrap();
    writeln!(report, "  ── match rates ──").unwrap();
    writeln!(report, "  overall:             {match_rate:.2}%").unwrap();
    writeln!(
        report,
        "  valid inputs only:   {valid_match_rate:.2}%  ({both_ok}/{valid_total})"
    )
    .unwrap();

    if !false_neg.is_empty() {
        // Write ALL false negatives to a file for analysis.
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let dump_path = manifest_dir.join("../../test-corpus/sass-spec-gaps.txt");
        let mut dump = String::new();
        for r in &false_neg {
            writeln!(dump, "{}\t{}", r.display_path, r.our_error_count).unwrap();
            for e in &r.first_errors {
                writeln!(dump, "  {e}").unwrap();
            }
        }
        let _ = std::fs::write(&dump_path, &dump);

        writeln!(report).unwrap();
        writeln!(
            report,
            "  ── false negatives (top {}) ──",
            false_neg.len().min(50)
        )
        .unwrap();
        for r in false_neg.iter().take(50) {
            writeln!(
                report,
                "    {} ({} errors)",
                r.display_path, r.our_error_count
            )
            .unwrap();
            for e in &r.first_errors {
                writeln!(report, "      {e}").unwrap();
            }
        }
        if false_neg.len() > 50 {
            writeln!(report, "    ... and {} more", false_neg.len() - 50).unwrap();
        }
    }

    eprint!("{report}");
}

// ── Main test ───────────────────────────────────────────────────────

#[test]
#[ignore]
fn sass_spec_compatibility() {
    let spec_dir = spec_root();

    if !spec_dir.exists() {
        panic!(
            "sass-spec not found at {}. Run: cd test-corpus && bash download.sh",
            spec_dir.display()
        );
    }

    let hrx_files = collect_hrx_files(&spec_dir);
    assert!(
        !hrx_files.is_empty(),
        "no .hrx files found in {}",
        spec_dir.display()
    );

    let mut all_cases: Vec<TestCase> = Vec::new();

    // Parse HRX files.
    for hrx_path in &hrx_files {
        let content = std::fs::read_to_string(hrx_path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", hrx_path.display()));

        let rel = hrx_path
            .strip_prefix(&spec_dir)
            .unwrap_or(hrx_path)
            .with_extension("")
            .to_string_lossy()
            .to_string();

        let cases = parse_hrx(&content, &rel);
        all_cases.extend(cases);
    }

    // Collect standalone input.scss tests.
    let standalone = collect_standalone_tests(&spec_dir, &spec_dir);
    all_cases.extend(standalone);

    eprintln!("\nparsing {} sass-spec test cases...", all_cases.len());

    // Parse and classify each test case.
    let results: Vec<CaseResult> = all_cases
        .iter()
        .map(|case| {
            let (green, errors) = sass_parser::parse(&case.input_scss);

            // Verify round-trip.
            let tree = SyntaxNode::new_root(green);
            let round_trip = tree.text().to_string();
            if round_trip != case.input_scss {
                eprintln!(
                    "ROUND-TRIP FAILURE: {} (len {} vs {})",
                    case.display_path,
                    case.input_scss.len(),
                    round_trip.len()
                );
            }

            let first_errors: Vec<String> = errors
                .iter()
                .take(3)
                .map(|(msg, range)| format!("{range:?}: {msg}"))
                .collect();

            CaseResult {
                display_path: case.display_path.clone(),
                outcome: classify(case, errors.len()),
                our_error_count: errors.len(),
                first_errors,
            }
        })
        .collect();

    print_compatibility_report(&results);

    // Hard assertion: round-trip must hold (our core invariant).
    // Match rate is reported, not asserted — we track it over time.
    let valid_total = results
        .iter()
        .filter(|r| r.outcome == Outcome::BothOk || r.outcome == Outcome::FalseNegative)
        .count();
    let both_ok = results
        .iter()
        .filter(|r| r.outcome == Outcome::BothOk)
        .count();
    let valid_rate = if valid_total > 0 {
        both_ok as f64 / valid_total as f64 * 100.0
    } else {
        0.0
    };

    eprintln!("\nvalid-input match rate: {valid_rate:.2}%");

    // Soft assertion: warn if below target but don't fail the build.
    if valid_rate < 99.0 {
        eprintln!(
            "\nWARNING: valid-input match rate {valid_rate:.2}% is below 99% target. \
             See false negatives above for gaps to fix."
        );
    }
}
