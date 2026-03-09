use std::path::Path;
use std::sync::LazyLock;

use divan::Bencher;
use sass_parser::syntax::SyntaxNode;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

static NORMALIZE_CSS: &str = include_str!("../tests/fixtures/normalize.css");

static ANGULAR_MATERIAL: LazyLock<String> = LazyLock::new(|| {
    let corpus_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../test-corpus/angular-material/scss");
    if !corpus_dir.exists() {
        return String::new();
    }
    let mut buf = String::new();
    collect_scss_recursive(&corpus_dir, &mut buf);
    buf
});

fn collect_scss_recursive(dir: &Path, out: &mut String) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<_> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect_scss_recursive(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "scss") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                out.push_str(&content);
                out.push('\n');
            }
        }
    }
}

// ── normalize.css (13 KB baseline) ─────────────────────────────────────

#[divan::bench]
fn lex_normalize_css(bencher: Bencher<'_, '_>) {
    bencher
        .counter(divan::counter::BytesCount::of_str(NORMALIZE_CSS))
        .with_inputs(|| NORMALIZE_CSS)
        .bench_values(|source| sass_parser::lexer::tokenize(source));
}

#[divan::bench]
fn lex_and_build_input_normalize_css(bencher: Bencher<'_, '_>) {
    bencher
        .counter(divan::counter::BytesCount::of_str(NORMALIZE_CSS))
        .with_inputs(|| NORMALIZE_CSS)
        .bench_values(|source| sass_parser::input::Input::from_source(source));
}

#[divan::bench]
fn parse_events_only_normalize_css(bencher: Bencher<'_, '_>) {
    bencher
        .counter(divan::counter::BytesCount::of_str(NORMALIZE_CSS))
        .with_inputs(|| NORMALIZE_CSS)
        .bench_values(|source| {
            let input = sass_parser::input::Input::from_source(source);
            let mut parser = sass_parser::parser::Parser::new(input, source);
            sass_parser::grammar::source_file(&mut parser);
            parser.finish()
        });
}

#[divan::bench]
fn parse_normalize_css(bencher: Bencher<'_, '_>) {
    bencher
        .counter(divan::counter::BytesCount::of_str(NORMALIZE_CSS))
        .with_inputs(|| NORMALIZE_CSS)
        .bench_values(|source| sass_parser::parse(source));
}

#[divan::bench]
fn parse_and_build_tree_normalize_css(bencher: Bencher<'_, '_>) {
    bencher
        .counter(divan::counter::BytesCount::of_str(NORMALIZE_CSS))
        .with_inputs(|| NORMALIZE_CSS)
        .bench_values(|source| {
            let (green, _) = sass_parser::parse(source);
            SyntaxNode::new_root(green)
        });
}

// ── Angular Material (~1.6 MB corpus) ──────────────────────────────────

#[divan::bench]
fn lex_angular_material(bencher: Bencher<'_, '_>) {
    let source = &*ANGULAR_MATERIAL;
    if source.is_empty() {
        return;
    }
    bencher
        .counter(divan::counter::BytesCount::of_str(source))
        .with_inputs(|| source.as_str())
        .bench_values(|s| sass_parser::lexer::tokenize(s));
}

#[divan::bench]
fn lex_and_build_input_angular_material(bencher: Bencher<'_, '_>) {
    let source = &*ANGULAR_MATERIAL;
    if source.is_empty() {
        return;
    }
    bencher
        .counter(divan::counter::BytesCount::of_str(source))
        .with_inputs(|| source.as_str())
        .bench_values(|s| sass_parser::input::Input::from_source(s));
}

#[divan::bench]
fn parse_events_only_angular_material(bencher: Bencher<'_, '_>) {
    let source = &*ANGULAR_MATERIAL;
    if source.is_empty() {
        return;
    }
    bencher
        .counter(divan::counter::BytesCount::of_str(source))
        .with_inputs(|| source.as_str())
        .bench_values(|s| {
            let input = sass_parser::input::Input::from_source(s);
            let mut parser = sass_parser::parser::Parser::new(input, s);
            sass_parser::grammar::source_file(&mut parser);
            parser.finish()
        });
}

#[divan::bench]
fn parse_angular_material(bencher: Bencher<'_, '_>) {
    let source = &*ANGULAR_MATERIAL;
    if source.is_empty() {
        return;
    }
    bencher
        .counter(divan::counter::BytesCount::of_str(source))
        .with_inputs(|| source.as_str())
        .bench_values(|s| sass_parser::parse(s));
}

#[divan::bench]
fn parse_and_build_tree_angular_material(bencher: Bencher<'_, '_>) {
    let source = &*ANGULAR_MATERIAL;
    if source.is_empty() {
        return;
    }
    bencher
        .counter(divan::counter::BytesCount::of_str(source))
        .with_inputs(|| source.as_str())
        .bench_values(|s| {
            let (green, _) = sass_parser::parse(s);
            SyntaxNode::new_root(green)
        });
}

fn main() {
    divan::main();
}
