use std::env;
use std::process;

use sass_parser::syntax::{SyntaxNode, debug_tree};

fn main() {
    let Some(path) = env::args().nth(1) else {
        eprintln!("usage: sass-parser <file.scss>");
        process::exit(1);
    };

    let source = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error reading {path}: {e}");
            process::exit(1);
        }
    };

    let (green, errors) = sass_parser::parse(&source);
    let tree = SyntaxNode::new_root(green);

    print!("{}", debug_tree(&tree));

    if !errors.is_empty() {
        eprintln!("errors:");
        for (msg, range) in &errors {
            eprintln!("  {range:?}: {msg}");
        }
        process::exit(2);
    }
}
