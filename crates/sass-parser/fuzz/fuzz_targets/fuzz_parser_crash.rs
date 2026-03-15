#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // Must not panic on any valid UTF-8 input.
        let _ = sass_parser::parse_scss(s);
    }
});
