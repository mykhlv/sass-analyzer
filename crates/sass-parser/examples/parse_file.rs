use std::env;
use std::process;

use sass_parser::syntax::{SyntaxNode, debug_tree};

// Case-sensitive intentionally — matches filesystem behavior on Linux and is
// consistent with sass-lsp's worker::is_sass_file.
#[allow(clippy::case_sensitive_file_extension_comparisons)]
fn main() {
    let Some(path) = env::args().nth(1) else {
        eprintln!("Usage: parse_file <path.scss|path.sass>");
        process::exit(1);
    };

    let source = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read {path}: {e}");
            process::exit(1);
        }
    };

    let (green, errors) = if path.ends_with(".sass") {
        sass_parser::parse_sass(&source)
    } else {
        sass_parser::parse_scss(&source)
    };
    let tree = SyntaxNode::new_root(green);

    // Print the full syntax tree (S-expression format)
    print!("{}", debug_tree(&tree));

    // Print any parse errors
    if !errors.is_empty() {
        eprintln!("\n{} error(s):", errors.len());
        for (msg, range) in &errors {
            eprintln!("  [{range:?}] {msg}");
        }
        process::exit(2);
    }
}
