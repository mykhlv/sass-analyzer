use sass_parser::reparse::{incremental_reparse, TextEdit};
use sass_parser::syntax::{debug_tree, SyntaxNode};
use sass_parser::text_range::TextSize;

fn apply_edit(source: &str, offset: u32, delete: u32, insert: &str) -> String {
    let mut s = source.to_owned();
    s.replace_range(offset as usize..(offset + delete) as usize, insert);
    s
}

/// Oracle: verify incremental reparse matches full reparse.
/// Returns true if incremental succeeded, false if it fell back.
fn check(old: &str, offset: u32, delete: u32, insert: &str) -> bool {
    let new_source = apply_edit(old, offset, delete, insert);
    let (old_green, old_errors) = sass_parser::parse(old);
    let (full_green, full_errors) = sass_parser::parse(&new_source);

    let edit = TextEdit {
        offset: TextSize::from(offset),
        delete: TextSize::from(delete),
        insert_len: TextSize::from(insert.len() as u32),
    };

    match incremental_reparse(&old_green, &old_errors, &edit, &new_source) {
        Some((incr_green, incr_errors)) => {
            let full_tree = SyntaxNode::new_root(full_green);
            let incr_tree = SyntaxNode::new_root(incr_green);

            assert_eq!(
                incr_tree.text().to_string(),
                full_tree.text().to_string(),
                "text mismatch for edit at {offset}..{} insert={insert:?}",
                offset + delete,
            );
            assert_eq!(
                debug_tree(&incr_tree),
                debug_tree(&full_tree),
                "tree structure mismatch for edit at {offset}..{} insert={insert:?}",
                offset + delete,
            );
            assert_eq!(
                incr_errors.len(),
                full_errors.len(),
                "error count mismatch for edit at {offset}..{} insert={insert:?}",
                offset + delete,
            );
            true
        }
        None => false,
    }
}

// ── Single-character edits in property values ───────────────────────

#[test]
fn insert_char_in_value() {
    assert!(check(".a { color: red; }", 15, 0, "d"));
}

#[test]
fn delete_char_in_value() {
    assert!(check(".a { color: red; }", 14, 1, ""));
}

#[test]
fn replace_char_in_value() {
    assert!(check(".a { color: red; }", 12, 3, "blue"));
}

// ── Variable edits ──────────────────────────────────────────────────

#[test]
fn edit_variable_value() {
    assert!(check("$x: 1;\n.a { color: $x; }", 4, 1, "2"));
}

#[test]
fn edit_variable_name() {
    assert!(check("$color: red;\n.a { }", 1, 5, "bg"));
}

// ── Declaration insertion ───────────────────────────────────────────

#[test]
fn insert_declaration() {
    assert!(check(
        ".a { color: red; }",
        16,
        0,
        "\n  font: bold;"
    ));
}

// ── Top-level edits ─────────────────────────────────────────────────

#[test]
fn edit_top_level_selector() {
    assert!(check(".a { color: red; }\n.b { }", 1, 1, "c"));
}

#[test]
fn add_rule_at_end() {
    // Single-item file — affected range covers all children, fallback expected.
    check(".a { color: red; }", 18, 0, "\n.b { font: bold; }");
}

// ── Edit at boundaries ──────────────────────────────────────────────

#[test]
fn insert_at_file_start() {
    // Single-item file — affected range covers all children, fallback expected.
    check(".a { }", 0, 0, "$x: 1;\n");
}

#[test]
fn insert_at_file_end() {
    // Single-item file — affected range covers all children, fallback expected.
    check(".a { }", 6, 0, "\n.b { }");
}

// ── Brace edits → expect fallback ───────────────────────────────────

#[test]
fn delete_closing_brace_falls_back() {
    // Deleting `}` is a structural change — incremental should fall back
    let result = check(".a { color: red; }", 17, 1, "");
    // Fallback is acceptable (the full reparse is always correct)
    let _ = result;
}

#[test]
fn delete_opening_brace_falls_back() {
    let result = check(".a { color: red; }", 3, 1, "");
    let _ = result;
}

// ── String context → may fall back ──────────────────────────────────

#[test]
fn edit_inside_string() {
    // Edit inside a string — may or may not fall back depending on
    // whether the string is inside a declaration (which is a direct
    // child of BLOCK). Either way, oracle ensures correctness.
    check("$x: \"hello\";\n.a { }", 8, 0, " world");
}

// ── Multi-rule file ─────────────────────────────────────────────────

#[test]
fn edit_middle_of_multi_rule() {
    let source = ".a { color: red; }\n.b { font: bold; }\n.c { margin: 0; }";
    assert!(check(source, 30, 4, "italic"));
}

// ── Nested rules ────────────────────────────────────────────────────

#[test]
fn edit_in_nested_block() {
    assert!(check(
        ".parent {\n  .child {\n    color: red;\n  }\n}",
        30,
        3,
        "blue"
    ));
}

// ── At-rules ────────────────────────────────────────────────────────

#[test]
fn edit_mixin_body() {
    assert!(check(
        "@mixin btn {\n  color: red;\n}",
        20,
        3,
        "blue"
    ));
}

#[test]
fn edit_function_body() {
    assert!(check(
        "@function double($n) {\n  @return $n * 2;\n}",
        35,
        1,
        "3"
    ));
}

// ── Error recovery ──────────────────────────────────────────────────

#[test]
fn edit_with_existing_error() {
    // File has a parse error, but edit is in a valid part
    check(".a { color: ; }\n.b { font: bold; }", 28, 4, "italic");
}

// ── Large file simulation ───────────────────────────────────────────

#[test]
fn large_file_single_edit() {
    let mut source = String::new();
    for i in 0..200 {
        source.push_str(&format!(".rule-{i} {{ color: val-{i}; }}\n"));
    }

    // Edit a value in the middle
    let target = "val-100";
    let offset = source.find(target).unwrap() as u32;
    assert!(check(&source, offset, target.len() as u32, "changed"));
}

// ── Whitespace-only edits ───────────────────────────────────────────

#[test]
fn add_blank_line() {
    assert!(check(
        ".a { color: red; }\n.b { font: bold; }",
        18,
        0,
        "\n\n"
    ));
}

// ── Multiple declarations ───────────────────────────────────────────

#[test]
fn edit_second_declaration() {
    assert!(check(
        ".a {\n  color: red;\n  font: bold;\n  margin: 0;\n}",
        26,
        4,
        "italic"
    ));
}
