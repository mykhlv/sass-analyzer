use std::path::{Path, PathBuf};

use sass_parser::resolver::{BuiltinModule, ModuleResolver, ResolveError, ResolvedModule};
use sass_parser::vfs::MemoryFs;

fn resolver_with(files: &[&str]) -> ModuleResolver<MemoryFs> {
    let mut fs = MemoryFs::new();
    for &path in files {
        fs.add(path, "");
    }
    ModuleResolver::with_vfs(fs)
}

// ── Basic resolution ────────────────────────────────────────────────

#[test]
fn resolve_explicit_scss() {
    let r = resolver_with(&["/project/src/colors.scss"]);
    assert_eq!(
        r.resolve("colors", Path::new("/project/src/main.scss")),
        Ok(ResolvedModule::File("/project/src/colors.scss".into())),
    );
}

#[test]
fn resolve_partial() {
    let r = resolver_with(&["/project/src/_colors.scss"]);
    assert_eq!(
        r.resolve("colors", Path::new("/project/src/main.scss")),
        Ok(ResolvedModule::File("/project/src/_colors.scss".into())),
    );
}

#[test]
fn resolve_index() {
    let r = resolver_with(&["/project/src/utils/index.scss"]);
    assert_eq!(
        r.resolve("utils", Path::new("/project/src/main.scss")),
        Ok(ResolvedModule::File("/project/src/utils/index.scss".into())),
    );
}

#[test]
fn resolve_partial_index() {
    let r = resolver_with(&["/project/src/utils/_index.scss"]);
    assert_eq!(
        r.resolve("utils", Path::new("/project/src/main.scss")),
        Ok(ResolvedModule::File(
            "/project/src/utils/_index.scss".into()
        )),
    );
}

#[test]
fn resolve_priority_non_partial_first() {
    // When both colors.scss and _colors.scss exist, non-partial wins
    let r = resolver_with(&["/project/src/colors.scss", "/project/src/_colors.scss"]);
    assert_eq!(
        r.resolve("colors", Path::new("/project/src/main.scss")),
        Ok(ResolvedModule::File("/project/src/colors.scss".into())),
    );
}

#[test]
fn resolve_subdirectory_spec() {
    let r = resolver_with(&["/project/src/shared/_vars.scss"]);
    assert_eq!(
        r.resolve("shared/vars", Path::new("/project/src/main.scss")),
        Ok(ResolvedModule::File(
            "/project/src/shared/_vars.scss".into()
        )),
    );
}

// ── Built-in modules ────────────────────────────────────────────────

#[test]
fn resolve_builtin_math() {
    let r = resolver_with(&[]);
    assert_eq!(
        r.resolve("sass:math", Path::new("/project/src/main.scss")),
        Ok(ResolvedModule::Builtin(BuiltinModule::Math)),
    );
}

#[test]
fn resolve_builtin_color() {
    let r = resolver_with(&[]);
    assert_eq!(
        r.resolve("sass:color", Path::new("/project/src/main.scss")),
        Ok(ResolvedModule::Builtin(BuiltinModule::Color)),
    );
}

#[test]
fn resolve_builtin_all_modules() {
    let r = resolver_with(&[]);
    let base = Path::new("/x.scss");
    assert_eq!(
        r.resolve("sass:list", base),
        Ok(ResolvedModule::Builtin(BuiltinModule::List))
    );
    assert_eq!(
        r.resolve("sass:map", base),
        Ok(ResolvedModule::Builtin(BuiltinModule::Map))
    );
    assert_eq!(
        r.resolve("sass:selector", base),
        Ok(ResolvedModule::Builtin(BuiltinModule::Selector)),
    );
    assert_eq!(
        r.resolve("sass:string", base),
        Ok(ResolvedModule::Builtin(BuiltinModule::SassString)),
    );
    assert_eq!(
        r.resolve("sass:meta", base),
        Ok(ResolvedModule::Builtin(BuiltinModule::Meta))
    );
}

#[test]
fn resolve_unknown_builtin() {
    let r = resolver_with(&[]);
    assert_eq!(
        r.resolve("sass:nope", Path::new("/x.scss")),
        Err(ResolveError::UnknownBuiltin("nope".into())),
    );
}

// ── CSS imports ─────────────────────────────────────────────────────

#[test]
fn resolve_css_import() {
    let r = resolver_with(&[]);
    assert_eq!(
        r.resolve("reset.css", Path::new("/project/src/main.scss")),
        Ok(ResolvedModule::Css("reset.css".into())),
    );
}

// ── Load paths ──────────────────────────────────────────────────────

#[test]
fn resolve_via_load_path() {
    let mut r = resolver_with(&["/libs/shared/_vars.scss"]);
    r.add_load_path("/libs");
    assert_eq!(
        r.resolve("shared/vars", Path::new("/project/src/main.scss")),
        Ok(ResolvedModule::File("/libs/shared/_vars.scss".into())),
    );
}

#[test]
fn resolve_relative_before_load_path() {
    // Relative resolution takes priority over load paths
    let mut r = resolver_with(&["/project/src/_colors.scss", "/libs/_colors.scss"]);
    r.add_load_path("/libs");
    assert_eq!(
        r.resolve("colors", Path::new("/project/src/main.scss")),
        Ok(ResolvedModule::File("/project/src/_colors.scss".into())),
    );
}

// ── Import aliases ──────────────────────────────────────────────────

#[test]
fn resolve_alias_single_target() {
    let mut r = resolver_with(&["/project/src/sass/colors.scss"]);
    r.add_import_alias("@sass".into(), vec![PathBuf::from("/project/src/sass")]);
    assert_eq!(
        r.resolve("@sass/colors", Path::new("/project/app/main.scss")),
        Ok(ResolvedModule::File("/project/src/sass/colors.scss".into())),
    );
}

#[test]
fn resolve_alias_partial() {
    let mut r = resolver_with(&["/project/src/sass/_mixins.scss"]);
    r.add_import_alias("@sass".into(), vec![PathBuf::from("/project/src/sass")]);
    assert_eq!(
        r.resolve("@sass/mixins", Path::new("/project/app/main.scss")),
        Ok(ResolvedModule::File(
            "/project/src/sass/_mixins.scss".into()
        )),
    );
}

#[test]
fn resolve_alias_with_extension() {
    let mut r = resolver_with(&["/project/src/sass/colors.scss"]);
    r.add_import_alias("@sass".into(), vec![PathBuf::from("/project/src/sass")]);
    assert_eq!(
        r.resolve("@sass/colors.scss", Path::new("/project/app/main.scss")),
        Ok(ResolvedModule::File("/project/src/sass/colors.scss".into())),
    );
}

#[test]
fn resolve_alias_with_extension_partial() {
    let mut r = resolver_with(&["/project/src/sass/_typography.scss"]);
    r.add_import_alias("@sass".into(), vec![PathBuf::from("/project/src/sass")]);
    assert_eq!(
        r.resolve("@sass/typography.scss", Path::new("/project/app/main.scss"),),
        Ok(ResolvedModule::File(
            "/project/src/sass/_typography.scss".into()
        )),
    );
}

#[test]
fn resolve_alias_multiple_targets_picks_closest() {
    let mut r = resolver_with(&[
        "/project/frontend/src/Sass/colors.scss",
        "/project/solutions/src/Sass/colors.scss",
    ]);
    r.add_import_alias(
        "@sass".into(),
        vec![
            PathBuf::from("/project/frontend/src/Sass"),
            PathBuf::from("/project/solutions/src/Sass"),
        ],
    );
    assert_eq!(
        r.resolve("@sass/colors", Path::new("/project/frontend/app/main.scss"),),
        Ok(ResolvedModule::File(
            "/project/frontend/src/Sass/colors.scss".into()
        )),
    );
    assert_eq!(
        r.resolve(
            "@sass/colors",
            Path::new("/project/solutions/app/main.scss"),
        ),
        Ok(ResolvedModule::File(
            "/project/solutions/src/Sass/colors.scss".into()
        )),
    );
}

#[test]
fn resolve_alias_prefix_boundary() {
    let mut r = resolver_with(&["/project/src/sass/x.scss"]);
    r.add_import_alias("@sass".into(), vec![PathBuf::from("/project/src/sass")]);
    // "@sass-utils/x" should NOT match the "@sass" alias
    assert_eq!(
        r.resolve("@sass-utils/x", Path::new("/project/main.scss")),
        Err(ResolveError::NotFound("@sass-utils/x".into())),
    );
}

#[test]
fn resolve_alias_not_found() {
    let mut r = resolver_with(&[]);
    r.add_import_alias("@sass".into(), vec![PathBuf::from("/project/src/sass")]);
    assert_eq!(
        r.resolve("@sass/nope", Path::new("/project/app/main.scss")),
        Err(ResolveError::NotFound("@sass/nope".into())),
    );
}

#[test]
fn resolve_alias_before_relative() {
    let mut r = resolver_with(&["/project/src/sass/colors.scss", "/project/app/colors.scss"]);
    r.add_import_alias("@sass".into(), vec![PathBuf::from("/project/src/sass")]);
    assert_eq!(
        r.resolve("@sass/colors", Path::new("/project/app/main.scss")),
        Ok(ResolvedModule::File("/project/src/sass/colors.scss".into())),
    );
}

// ── node_modules resolution ─────────────────────────────────────────

#[test]
fn resolve_node_modules_direct() {
    let mut r = resolver_with(&["/project/node_modules/@quartznetwork/sass/colors.scss"]);
    r.enable_node_modules();
    assert_eq!(
        r.resolve(
            "@quartznetwork/sass/colors",
            Path::new("/project/src/main.scss"),
        ),
        Ok(ResolvedModule::File(
            "/project/node_modules/@quartznetwork/sass/colors.scss".into(),
        )),
    );
}

#[test]
fn resolve_node_modules_walks_up() {
    let mut r = resolver_with(&["/project/node_modules/shared-styles/_vars.scss"]);
    r.enable_node_modules();
    assert_eq!(
        r.resolve(
            "shared-styles/vars",
            Path::new("/project/src/deep/nested/main.scss"),
        ),
        Ok(ResolvedModule::File(
            "/project/node_modules/shared-styles/_vars.scss".into(),
        )),
    );
}

#[test]
fn resolve_node_modules_index() {
    let mut r = resolver_with(&["/project/node_modules/@org/pkg/_index.scss"]);
    r.enable_node_modules();
    assert_eq!(
        r.resolve("@org/pkg", Path::new("/project/src/main.scss")),
        Ok(ResolvedModule::File(
            "/project/node_modules/@org/pkg/_index.scss".into(),
        )),
    );
}

#[test]
fn resolve_load_paths_before_node_modules() {
    let mut r = resolver_with(&["/libs/_colors.scss", "/project/node_modules/colors.scss"]);
    r.add_load_path("/libs");
    r.enable_node_modules();
    assert_eq!(
        r.resolve("colors", Path::new("/project/src/main.scss")),
        Ok(ResolvedModule::File("/libs/_colors.scss".into())),
    );
}

// ── Not found ───────────────────────────────────────────────────────

#[test]
fn resolve_not_found() {
    let r = resolver_with(&[]);
    assert_eq!(
        r.resolve("nope", Path::new("/project/src/main.scss")),
        Err(ResolveError::NotFound("nope".into())),
    );
}

// ── AST module_path() helpers ───────────────────────────────────────

#[test]
fn use_rule_module_path() {
    let (green, _) = sass_parser::parse_scss("@use \"sass:math\";");
    let root = sass_parser::syntax::SyntaxNode::new_root(green);
    let use_rule = root
        .children()
        .find_map(sass_parser::ast::UseRule::cast)
        .expect("should have UseRule");
    assert_eq!(use_rule.module_path(), Some("sass:math".into()));
}

#[test]
fn forward_rule_module_path() {
    let (green, _) = sass_parser::parse_scss("@forward \"colors\";");
    let root = sass_parser::syntax::SyntaxNode::new_root(green);
    let fwd = root
        .children()
        .find_map(sass_parser::ast::ForwardRule::cast)
        .expect("should have ForwardRule");
    assert_eq!(fwd.module_path(), Some("colors".into()));
}

use sass_parser::ast::AstNode;

// ── .sass file resolution ───────────────────────────────────────────

#[test]
fn resolve_sass_extension() {
    let r = resolver_with(&["/project/src/colors.sass"]);
    assert_eq!(
        r.resolve("colors", Path::new("/project/src/main.scss")),
        Ok(ResolvedModule::File("/project/src/colors.sass".into())),
    );
}

#[test]
fn resolve_sass_partial() {
    let r = resolver_with(&["/project/src/_colors.sass"]);
    assert_eq!(
        r.resolve("colors", Path::new("/project/src/main.scss")),
        Ok(ResolvedModule::File("/project/src/_colors.sass".into())),
    );
}

#[test]
fn resolve_sass_index() {
    let r = resolver_with(&["/project/src/utils/index.sass"]);
    assert_eq!(
        r.resolve("utils", Path::new("/project/src/main.scss")),
        Ok(ResolvedModule::File("/project/src/utils/index.sass".into())),
    );
}

#[test]
fn resolve_sass_partial_index() {
    let r = resolver_with(&["/project/src/utils/_index.sass"]);
    assert_eq!(
        r.resolve("utils", Path::new("/project/src/main.scss")),
        Ok(ResolvedModule::File(
            "/project/src/utils/_index.sass".into()
        )),
    );
}

#[test]
fn resolve_scss_preferred_over_sass() {
    // When both .scss and .sass exist, .scss wins (more common)
    let r = resolver_with(&["/project/src/colors.scss", "/project/src/colors.sass"]);
    assert_eq!(
        r.resolve("colors", Path::new("/project/src/main.scss")),
        Ok(ResolvedModule::File("/project/src/colors.scss".into())),
    );
}

#[test]
fn resolve_explicit_sass_extension() {
    let r = resolver_with(&["/project/src/colors.sass"]);
    assert_eq!(
        r.resolve("colors.sass", Path::new("/project/src/main.scss")),
        Ok(ResolvedModule::File("/project/src/colors.sass".into())),
    );
}

// ── Dart Sass candidate order ───────────────────────────────────────

fn resolves(r: &ModuleResolver<MemoryFs>, spec: &str) -> Result<ResolvedModule, ResolveError> {
    r.resolve(spec, Path::new("/p/main.scss"))
}

fn resolves_for_import(
    r: &ModuleResolver<MemoryFs>,
    spec: &str,
) -> Result<ResolvedModule, ResolveError> {
    r.resolve_for_import(spec, Path::new("/p/main.scss"))
}

// Compared against `resolve`'s result, so it has its type.
#[allow(clippy::unnecessary_wraps)]
fn file(path: &str) -> Result<ResolvedModule, ResolveError> {
    Ok(ResolvedModule::File(PathBuf::from(path)))
}

#[test]
fn resolve_css_file_without_extension() {
    assert_eq!(
        resolves(&resolver_with(&["/p/foo.css"]), "foo"),
        file("/p/foo.css")
    );
    assert_eq!(
        resolves(&resolver_with(&["/p/_foo.css"]), "foo"),
        file("/p/_foo.css")
    );
    assert_eq!(
        resolves_for_import(&resolver_with(&["/p/foo.css"]), "foo"),
        file("/p/foo.css")
    );
}

#[test]
fn resolve_stylesheet_before_css() {
    let r = resolver_with(&["/p/foo.css", "/p/_foo.sass"]);
    assert_eq!(resolves(&r, "foo"), file("/p/_foo.sass"));
}

#[test]
fn resolve_css_file_before_directory_index() {
    let r = resolver_with(&["/p/foo.css", "/p/foo/_index.scss"]);
    assert_eq!(resolves(&r, "foo"), file("/p/foo.css"));
}

#[test]
fn resolve_css_index() {
    assert_eq!(
        resolves(&resolver_with(&["/p/foo/index.css"]), "foo"),
        file("/p/foo/index.css")
    );
    assert_eq!(
        resolves(&resolver_with(&["/p/foo/_index.css"]), "foo"),
        file("/p/foo/_index.css")
    );
}

#[test]
fn resolve_css_in_a_dotted_package_directory() {
    // `@import "normalize.css/normalize"` with `node_modules` as a load path.
    let mut r = resolver_with(&["/p/node_modules/normalize.css/normalize.css"]);
    r.add_load_path("/p/node_modules");
    assert_eq!(
        resolves_for_import(&r, "normalize.css/normalize"),
        file("/p/node_modules/normalize.css/normalize.css")
    );
}

#[test]
fn import_only_file_is_preferred_by_import() {
    let r = resolver_with(&["/p/_bar.import.scss", "/p/_bar.scss"]);
    assert_eq!(resolves_for_import(&r, "bar"), file("/p/_bar.import.scss"));
    assert_eq!(resolves(&r, "bar"), file("/p/_bar.scss"));
}

#[test]
fn import_only_file_is_invisible_to_use() {
    let r = resolver_with(&["/p/_bar.import.scss", "/p/bar/_index.import.scss"]);
    assert_eq!(
        resolves(&r, "bar"),
        Err(ResolveError::NotFound("bar".into()))
    );
}

#[test]
fn import_only_file_with_explicit_extension() {
    let r = resolver_with(&["/p/_bar.import.scss", "/p/_bar.scss"]);
    assert_eq!(
        resolves_for_import(&r, "bar.scss"),
        file("/p/_bar.import.scss")
    );
}

#[test]
fn import_only_file_before_directory_index() {
    let r = resolver_with(&["/p/bar.import.sass", "/p/bar/_index.scss"]);
    assert_eq!(resolves_for_import(&r, "bar"), file("/p/bar.import.sass"));
}

#[test]
fn import_only_index() {
    let r = resolver_with(&["/p/bar/_index.import.scss", "/p/bar/_index.scss"]);
    assert_eq!(
        resolves_for_import(&r, "bar"),
        file("/p/bar/_index.import.scss")
    );
    assert_eq!(resolves(&r, "bar"), file("/p/bar/_index.scss"));
}

#[test]
fn resolve_percent_encoded_url() {
    let r = resolver_with(&["/p/my file.scss", "/p/my dir/_a.scss"]);
    assert_eq!(resolves(&r, "my%20file"), file("/p/my file.scss"));
    assert_eq!(resolves(&r, "my%20dir/a"), file("/p/my dir/_a.scss"));
}

#[test]
fn invalid_percent_escape_is_kept() {
    let r = resolver_with(&["/p/100%.scss", "/p/a%zz.scss"]);
    assert_eq!(resolves(&r, "100%"), file("/p/100%.scss"));
    assert_eq!(resolves(&r, "a%zz"), file("/p/a%zz.scss"));
}

#[test]
fn ambiguous_candidates_still_resolve() {
    // Dart Sass reports these as ambiguous; picking one keeps them from
    // looking missing.
    let r = resolver_with(&["/p/_a.scss", "/p/a.sass"]);
    assert_eq!(resolves(&r, "a"), file("/p/_a.scss"));
    let r = resolver_with(&["/p/a/_index.scss", "/p/a/index.scss"]);
    assert_eq!(resolves(&r, "a"), file("/p/a/index.scss"));
    let r = resolver_with(&["/p/foo.css", "/p/_foo.css"]);
    assert_eq!(resolves(&r, "foo"), file("/p/foo.css"));
}

#[test]
fn file_before_directory_index() {
    let r = resolver_with(&["/p/a.scss", "/p/a/_index.scss"]);
    assert_eq!(resolves(&r, "a"), file("/p/a.scss"));
}
