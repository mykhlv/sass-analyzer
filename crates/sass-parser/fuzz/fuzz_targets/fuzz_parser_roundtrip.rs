#![no_main]

use libfuzzer_sys::fuzz_target;
use sass_parser::syntax::SyntaxNode;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let (green, _) = sass_parser::parse(s);
        let tree = SyntaxNode::new_root(green);
        assert_eq!(
            tree.text().to_string(),
            s,
            "lossless round-trip failed for input: {s:?}"
        );
    }
});
