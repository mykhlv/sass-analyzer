use divan::Bencher;
use sass_parser::syntax::SyntaxNode;

static NORMALIZE_CSS: &str = include_str!("../tests/fixtures/normalize.css");

#[divan::bench]
fn lex_normalize_css(bencher: Bencher<'_, '_>) {
    bencher
        .with_inputs(|| NORMALIZE_CSS)
        .bench_values(|source| sass_parser::lexer::tokenize(source));
}

#[divan::bench]
fn parse_normalize_css(bencher: Bencher<'_, '_>) {
    bencher
        .with_inputs(|| NORMALIZE_CSS)
        .bench_values(|source| sass_parser::parse(source));
}

#[divan::bench]
fn parse_and_build_tree_normalize_css(bencher: Bencher<'_, '_>) {
    bencher
        .with_inputs(|| NORMALIZE_CSS)
        .bench_values(|source| {
            let (green, _) = sass_parser::parse(source);
            SyntaxNode::new_root(green)
        });
}

fn main() {
    divan::main();
}
