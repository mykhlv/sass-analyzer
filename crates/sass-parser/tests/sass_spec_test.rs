//! sass-spec compatibility harness (Task 5.15).
//!
//! Parses every `input.scss` and `input.sass` from the sass/sass-spec test
//! suite and compares parse success/failure against dart-sass expectations
//! (inferred from the presence of `output.css` vs `error` sibling entries).
//!
//! Download the spec first:
//!   cd test-corpus && bash download.sh sass-spec
//!
//! Run with:
//!   `cargo test --test sass_spec_test -- --ignored --nocapture`

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::path::{Path, PathBuf};

use sass_parser::syntax::SyntaxNode;

#[path = "common/layout_allow_list.rs"]
mod layout_allow_list;
#[path = "common/layout_differential.rs"]
mod layout_differential;
#[path = "common/line_facts.rs"]
mod line_facts;
#[path = "common/tree_shape.rs"]
mod tree_shape;
#[path = "common/virtual_tokens.rs"]
mod virtual_tokens;

use tree_shape::{Shape, describe_record_diff, describe_record_diff_with, record_key};

// ── HRX parser ──────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
enum Syntax {
    Scss,
    Sass,
}

/// A single test case extracted from an HRX file or a directory.
struct TestCase {
    /// Display path: `hrx_file_relative` / `entry_prefix`
    display_path: String,
    input: String,
    syntax: Syntax,
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
        } else if current_path.is_some() {
            if !current_body.is_empty() {
                current_body.push('\n');
            }
            current_body.push_str(line);
        }
    }
    // Flush last entry.
    if let Some(path) = current_path.take() {
        entries.insert(path, current_body);
    }

    // Group entries by prefix directory to find distinct test cases.
    // A test case has `{prefix/}input.scss` or `{prefix/}input.sass`.
    let input_keys: Vec<(String, Syntax)> = entries
        .keys()
        .filter_map(|k| {
            if k.ends_with("input.scss") {
                Some((k.clone(), Syntax::Scss))
            } else if k.ends_with("input.sass") {
                Some((k.clone(), Syntax::Sass))
            } else {
                None
            }
        })
        .collect();

    let mut cases = Vec::new();
    for (input_key, syntax) in &input_keys {
        let filename = match syntax {
            Syntax::Scss => "input.scss",
            Syntax::Sass => "input.sass",
        };
        let prefix = if input_key == filename {
            ""
        } else {
            &input_key[..input_key.len() - filename.len()]
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
            input: entries[input_key].clone(),
            syntax: *syntax,
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
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
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
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries {
        let Ok(entry) = entry else { continue };
        let path = entry.path();

        if path.is_dir() {
            collect_standalone_recursive(&path, spec_dir, out);
        } else {
            let syntax = if path.file_name().is_some_and(|n| n == "input.scss") {
                Syntax::Scss
            } else if path.file_name().is_some_and(|n| n == "input.sass") {
                Syntax::Sass
            } else {
                continue;
            };

            let parent = path.parent().unwrap();
            let Ok(source) = std::fs::read_to_string(&path) else {
                continue;
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
                input: source,
                syntax,
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
    /// Shape record, kept for the cases dart-sass accepts.
    shape: Option<String>,
    /// Noisy-error record, kept for the cases dart-sass rejects where we report
    /// more than [`MAX_DIAGNOSTICS`].
    noisy: Option<String>,
}

/// Most diagnostics one syntax error may produce before its recovery counts as a
/// cascade.
const MAX_DIAGNOSTICS: usize = 3;

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

fn print_compatibility_report(label: &str, results: &[CaseResult], dump_filename: &str) {
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
    writeln!(report, "  sass-spec compatibility report ({label})").unwrap();
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
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let dump_path = manifest_dir.join(format!("../../test-corpus/{dump_filename}"));
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

/// Every case of the suite, from HRX files and standalone inputs alike.
fn collect_all_cases() -> Vec<TestCase> {
    let spec_dir = spec_root();

    assert!(
        spec_dir.exists(),
        "sass-spec not found at {}. Run: cd test-corpus && bash download.sh",
        spec_dir.display()
    );

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
    all_cases
}

/// Every indented case has layout facts that reproduce the indentation the legacy
/// indented lexer measures at each newline, and the same comments.
#[test]
#[ignore = "requires downloading sass-spec corpus"]
fn sass_spec_line_facts_match_legacy() {
    let cases: Vec<TestCase> = collect_all_cases()
        .into_iter()
        .filter(|c| c.syntax == Syntax::Sass)
        .collect();
    let newlines: usize = cases
        .iter()
        .map(|c| sass_parser::sass_lexer::newline_indents(&c.input).len())
        .sum();
    let mismatches: Vec<String> = cases
        .iter()
        .flat_map(|c| {
            line_facts::line_fact_mismatches(&c.input)
                .into_iter()
                .map(|m| format!("{} {m}", c.display_path))
        })
        .collect();
    eprintln!(
        "\nline facts vs legacy: {} indented cases, {newlines} newlines checked, {} mismatches",
        cases.len(),
        mismatches.len(),
    );
    assert!(
        mismatches.is_empty(),
        "layout facts disagree with the legacy lexer: {:#?}",
        &mismatches[..mismatches.len().min(20)],
    );
}

/// Every indented case parses to the same tree when the legacy lexer's zero-width
/// tokens are emitted as `Event::Virtual` instead.
#[test]
#[ignore = "requires downloading sass-spec corpus"]
fn sass_spec_virtual_tokens_match_legacy() {
    let cases: Vec<TestCase> = collect_all_cases()
        .into_iter()
        .filter(|c| c.syntax == Syntax::Sass)
        .collect();
    let mismatches: Vec<String> = cases
        .iter()
        .filter_map(|c| {
            virtual_tokens::virtual_token_mismatch(&c.input)
                .map(|m| format!("{} {m}", c.display_path))
        })
        .collect();
    eprintln!(
        "\nEvent::Virtual vs legacy: {} indented cases, {} mismatches",
        cases.len(),
        mismatches.len(),
    );
    assert!(
        mismatches.is_empty(),
        "Event::Virtual disagrees with the legacy lexer: {:#?}",
        &mismatches[..mismatches.len().min(3)],
    );
}

/// Every indented case parses to the same tree and errors with the layout frontend as
/// with legacy, unless it is on the allow-list.
#[test]
#[ignore = "requires downloading sass-spec corpus"]
fn sass_spec_layout_matches_legacy() {
    let cases: Vec<TestCase> = collect_all_cases()
        .into_iter()
        .filter(|c| c.syntax == Syntax::Sass)
        .collect();
    let mut allowed = 0;
    let mut mismatches = Vec::new();
    for case in &cases {
        let Some((legacy, layout)) = layout_differential::layout_mismatch(&case.input) else {
            continue;
        };
        if layout_allow_list::allowed(&case.input).is_some() {
            allowed += 1;
        } else {
            mismatches.push(format!(
                "{}\n{}",
                case.display_path,
                layout_differential::first_difference(&legacy, &layout)
            ));
        }
    }
    eprintln!(
        "\nparse_sass vs parse_sass_legacy: {} indented cases, {allowed} \
         allow-listed differences, {} other",
        cases.len(),
        mismatches.len(),
    );
    assert!(
        mismatches.is_empty(),
        "parse_sass disagrees with parse_sass_legacy:\n{}",
        mismatches[..mismatches.len().min(3)].join("\n"),
    );
}

#[test]
#[ignore = "requires downloading sass-spec corpus"]
fn sass_spec_compatibility() {
    let all_cases = collect_all_cases();

    // Split into SCSS and Sass test cases.
    let (scss_cases, sass_cases): (Vec<_>, Vec<_>) = all_cases
        .into_iter()
        .partition(|c| c.syntax == Syntax::Scss);

    eprintln!(
        "\nparsing {} SCSS + {} Sass = {} sass-spec test cases...",
        scss_cases.len(),
        sass_cases.len(),
        scss_cases.len() + sass_cases.len()
    );

    let parse_and_classify = |cases: &[TestCase]| -> Vec<CaseResult> {
        cases
            .iter()
            .map(|case| {
                let (green, errors) = match case.syntax {
                    Syntax::Scss => sass_parser::parse_scss(&case.input),
                    Syntax::Sass => sass_parser::parse_sass(&case.input),
                };

                // Verify round-trip.
                let tree = SyntaxNode::new_root(green);
                let round_trip = tree.text().to_string();
                if round_trip != case.input {
                    eprintln!(
                        "ROUND-TRIP FAILURE: {} (len {} vs {})",
                        case.display_path,
                        case.input.len(),
                        round_trip.len()
                    );
                }

                let first_errors: Vec<String> = errors
                    .iter()
                    .take(3)
                    .map(|(msg, range)| format!("{range:?}: {msg}"))
                    .collect();

                let group = match case.syntax {
                    Syntax::Scss => "scss",
                    Syntax::Sass => "sass",
                };
                let shape = case
                    .expects_success
                    .then(|| Shape::of(&tree).record(group, &case.display_path, errors.len()));
                let noisy = (case.expects_error && errors.len() > MAX_DIAGNOSTICS).then(|| {
                    format!(
                        "{group}\t{}\t{}\t{}",
                        case.display_path.replace('\\', "/"),
                        errors.len(),
                        errors[0].0.replace(['\t', '\n'], " "),
                    )
                });

                CaseResult {
                    display_path: case.display_path.clone(),
                    outcome: classify(case, errors.len()),
                    our_error_count: errors.len(),
                    first_errors,
                    shape,
                    noisy,
                }
            })
            .collect()
    };

    let scss_results = parse_and_classify(&scss_cases);
    let sass_results = parse_and_classify(&sass_cases);

    print_compatibility_report("SCSS", &scss_results, "sass-spec-gaps.txt");
    if !sass_results.is_empty() {
        print_compatibility_report("Sass indented", &sass_results, "sass-spec-sass-gaps.txt");
    }

    // Summary: valid-input match rates for both syntaxes.
    for (label, results) in [("SCSS", &scss_results), ("Sass", &sass_results)] {
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

        eprintln!("\n{label} valid-input match rate: {valid_rate:.2}%");

        if label == "SCSS" && valid_rate < 99.0 {
            eprintln!(
                "\nWARNING: SCSS valid-input match rate {valid_rate:.2}% is below 99% target. \
                 See false negatives above for gaps to fix."
            );
        }
    }

    // Both baselines are checked before either fails, so one run reports every drift.
    let failures: Vec<String> = [
        check_shapes(scss_results.iter().chain(&sass_results)),
        check_noisy_errors(scss_results.iter().chain(&sass_results)),
    ]
    .into_iter()
    .flatten()
    .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Compare the tree shape of every case dart-sass accepts with the tracked
/// `sass-spec-shapes.txt`, returning a per-case diff on any difference.
///
/// The match rates above count a valid input as compatible whenever it parses
/// without errors, so a valid input misparsed with no diagnostic passes them.
/// Recording its shape catches that, for syntax the corpora do not cover.
fn check_shapes<'a>(results: impl Iterator<Item = &'a CaseResult>) -> Option<String> {
    const BASELINE: &str = "sass-spec-shapes.txt";
    const ACTUAL: &str = "sass-spec-shapes.actual.txt";

    let mut records: Vec<String> = results.filter_map(|r| r.shape.clone()).collect();
    records.sort();
    let keys: BTreeSet<&str> = records.iter().map(|r| record_key(r)).collect();
    assert_eq!(
        keys.len(),
        records.len(),
        "two sass-spec cases share a syntax and path, so their shapes cannot be told apart",
    );

    let mut out = format!(
        "# sass-spec tree-shape baseline: one record per case dart-sass accepts.\n\
         # Checked with:\n\
         #   cargo test -p sass-parser --test sass_spec_test -- --ignored sass_spec_compatibility\n\
         # A run that differs writes test-corpus/{ACTUAL} instead of this file;\n\
         # once every change in it is reviewed as intended, copy it over this one.\n\
         # Columns: syntax, case, parse errors, RULE_SET / DECLARATION / BLOCK node\n\
         # counts, and an FNV-1a hash of the case's debug_tree. Records are compared\n\
         # exactly: a valid input whose tree changes shape with no new error fails.\n"
    );
    for record in &records {
        out.push_str(record);
        out.push('\n');
    }

    let test_corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test-corpus");
    let baseline_path = test_corpus.join(BASELINE);
    let actual_path = test_corpus.join(ACTUAL);
    let committed = std::fs::read_to_string(&baseline_path).unwrap_or_default();
    let old: Vec<&str> = committed.lines().filter(|l| !l.starts_with('#')).collect();
    let diff = describe_record_diff(&old, &records);

    // On a difference, leave the tracked baseline untouched, as the corpus gates do,
    // so that a second run cannot read a regressed result back as the reference.
    if diff.is_none() {
        std::fs::write(&baseline_path, &out).expect("write shape baseline");
        let _ = std::fs::remove_file(&actual_path);
    } else {
        std::fs::write(&actual_path, &out).expect("write shape baseline");
    }
    eprintln!("\nsass-spec tree shapes: {} valid cases", records.len());

    diff.map(|diff| {
        format!(
            "sass-spec tree shapes no longer match {}:\n{diff}\
             Review `diff {} {}`. If every change is intended, copy the .actual \
             file over the baseline and commit it.",
            baseline_path.display(),
            baseline_path.display(),
            actual_path.display(),
        )
    })
}

/// Compare every case dart-sass rejects where we report more than
/// [`MAX_DIAGNOSTICS`] with the tracked `sass-spec-noisy-errors.txt`, returning a
/// per-case diff on any difference.
///
/// sass-spec only checks that an invalid input errors, so a recovery that turns one
/// typo into a cascade of diagnostics passes it. This baseline is a ratchet: a new
/// offender, a changed count and a fixed case all show up as a difference.
fn check_noisy_errors<'a>(results: impl Iterator<Item = &'a CaseResult>) -> Option<String> {
    const BASELINE: &str = "sass-spec-noisy-errors.txt";
    const ACTUAL: &str = "sass-spec-noisy-errors.actual.txt";
    const COLUMNS: [&str; 4] = ["syntax", "path", "errors", "first_error"];

    let mut records: Vec<String> = results.filter_map(|r| r.noisy.clone()).collect();
    records.sort();

    let mut out = format!(
        "# sass-spec noisy-error baseline: one record per case dart-sass rejects where\n\
         # the parser reports more than {MAX_DIAGNOSTICS} diagnostics.\n\
         # Checked with:\n\
         #   cargo test -p sass-parser --test sass_spec_test -- --ignored sass_spec_compatibility\n\
         # A run that differs writes test-corpus/{ACTUAL} instead of this file;\n\
         # once every change in it is reviewed as intended, copy it over this one.\n\
         # Columns: syntax, case, parse errors, first error message. A new offender,\n\
         # a changed count and a case that drops out all fail, so every change to this\n\
         # list is a reviewed one.\n"
    );
    for record in &records {
        out.push_str(record);
        out.push('\n');
    }

    let test_corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test-corpus");
    let baseline_path = test_corpus.join(BASELINE);
    let actual_path = test_corpus.join(ACTUAL);
    let committed = std::fs::read_to_string(&baseline_path).unwrap_or_default();
    let old: Vec<&str> = committed.lines().filter(|l| !l.starts_with('#')).collect();
    let diff = describe_record_diff_with(&old, &records, &COLUMNS, "now within the limit");

    if diff.is_none() {
        std::fs::write(&baseline_path, &out).expect("write noisy-error baseline");
        let _ = std::fs::remove_file(&actual_path);
    } else {
        std::fs::write(&actual_path, &out).expect("write noisy-error baseline");
    }
    eprintln!(
        "\nsass-spec noisy errors: {} cases with >{MAX_DIAGNOSTICS} diagnostics",
        records.len()
    );

    diff.map(|diff| {
        format!(
            "sass-spec noisy errors no longer match {}:\n{diff}\
             Review `diff {} {}`. If every change is intended, copy the .actual \
             file over the baseline and commit it.",
            baseline_path.display(),
            baseline_path.display(),
            actual_path.display(),
        )
    })
}
