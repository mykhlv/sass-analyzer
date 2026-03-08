#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let tokens = sass_parser::lexer::tokenize(s);
        let reconstructed: String = tokens.iter().map(|(_, text)| *text).collect();
        assert_eq!(
            reconstructed, s,
            "lossless round-trip failed for input: {s:?}"
        );
    }
});
