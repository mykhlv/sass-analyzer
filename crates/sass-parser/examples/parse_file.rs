use std::env;
use std::process;

use sass_parser::syntax::{SyntaxNode, debug_tree};

fn main() {
    let path = match env::args().nth(1) {
        Some(p) => p,
        None => {
            eprintln!("Usage: parse_file <path.scss|path.sass>");
            process::exit(1);
        }
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
