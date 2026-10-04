//! `parse_sass_layout` against `parse_sass_legacy`: the two indented frontends must give
//! the same tree and errors, except for the kinds of difference the layout frontend makes
//! on purpose, which the corpus tests know too (`tests/common/layout_classes.rs`). A
//! difference of no known kind does not stop the run: the input is recorded under
//! `artifacts/fuzz_sass_layout_differential/unclassified/`, for triage, and fuzzing goes
//! on. A lost round trip, or a panic in either frontend, still crashes.
//!
//! Run it for a fixed time while the two frontends coexist, not as a standing job:
//! `cargo +nightly fuzz run fuzz_sass_layout_differential -- -max_total_time=600`.

#![no_main]

use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;

use libfuzzer_sys::fuzz_target;

#[allow(dead_code)]
#[path = "../../tests/common/layout_differential.rs"]
mod layout_differential;

#[path = "../../tests/common/layout_classes.rs"]
mod layout_classes;

fuzz_target!(|data: &[u8]| {
    let Ok(source) = std::str::from_utf8(data) else {
        return;
    };
    let classes = layout_classes::classify(source);
    if classes.last() == Some(&layout_classes::Class::Other) {
        record_unclassified(data);
    }
});

/// Keep an input whose difference is of no known kind, once.
fn record_unclassified(data: &[u8]) {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("artifacts/fuzz_sass_layout_differential/unclassified");
    let mut hasher = DefaultHasher::new();
    data.hash(&mut hasher);
    let path = dir.join(format!("{:016x}", hasher.finish()));
    if path.exists() {
        return;
    }
    if std::fs::create_dir_all(&dir).is_ok() && std::fs::write(&path, data).is_ok() {
        eprintln!("difference of no known kind recorded at {}", path.display());
    }
}
