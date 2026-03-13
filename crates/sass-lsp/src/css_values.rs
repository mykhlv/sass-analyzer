/// Global CSS keywords valid for any property.
pub static GLOBAL_KEYWORDS: &[&str] = &["inherit", "initial", "revert", "revert-layer", "unset"];

/// Look up allowed keyword values for a CSS property.
/// Returns an empty slice for unknown or value-only properties.
pub fn values_for_property(name: &str) -> &'static [&'static str] {
    let name_lower = name.to_ascii_lowercase();
    match CSS_PROPERTY_VALUES.binary_search_by_key(&&*name_lower, |(prop, _)| *prop) {
        Ok(idx) => CSS_PROPERTY_VALUES[idx].1,
        Err(_) => &[],
    }
}

// Binary-search requires alphabetical sort by property name.
#[rustfmt::skip]
static CSS_PROPERTY_VALUES: &[(&str, &[&str])] = &[
    ("align-content",           &["center", "end", "flex-end", "flex-start", "normal", "space-around", "space-between", "space-evenly", "start", "stretch"]),
    ("align-items",             &["baseline", "center", "end", "flex-end", "flex-start", "normal", "self-end", "self-start", "start", "stretch"]),
    ("align-self",              &["auto", "baseline", "center", "end", "flex-end", "flex-start", "normal", "self-end", "self-start", "start", "stretch"]),
    ("animation-direction",     &["alternate", "alternate-reverse", "normal", "reverse"]),
    ("animation-fill-mode",     &["backwards", "both", "forwards", "none"]),
    ("animation-play-state",    &["paused", "running"]),
    ("animation-timing-function", &["ease", "ease-in", "ease-in-out", "ease-out", "linear", "step-end", "step-start"]),
    ("appearance",              &["auto", "menulist-button", "none", "textfield"]),
    ("backface-visibility",     &["hidden", "visible"]),
    ("background-attachment",   &["fixed", "local", "scroll"]),
    ("background-blend-mode",   &["color", "color-burn", "color-dodge", "darken", "difference", "exclusion", "hard-light", "hue", "lighten", "luminosity", "multiply", "normal", "overlay", "saturation", "screen", "soft-light"]),
    ("background-clip",         &["border-box", "content-box", "padding-box", "text"]),
    ("background-origin",       &["border-box", "content-box", "padding-box"]),
    ("background-repeat",       &["no-repeat", "repeat", "repeat-x", "repeat-y", "round", "space"]),
    ("background-size",         &["auto", "contain", "cover"]),
    ("border-block-style",      &["dashed", "dotted", "double", "groove", "hidden", "inset", "none", "outset", "ridge", "solid"]),
    ("border-bottom-style",     &["dashed", "dotted", "double", "groove", "hidden", "inset", "none", "outset", "ridge", "solid"]),
    ("border-collapse",         &["collapse", "separate"]),
    ("border-inline-style",     &["dashed", "dotted", "double", "groove", "hidden", "inset", "none", "outset", "ridge", "solid"]),
    ("border-left-style",       &["dashed", "dotted", "double", "groove", "hidden", "inset", "none", "outset", "ridge", "solid"]),
    ("border-right-style",      &["dashed", "dotted", "double", "groove", "hidden", "inset", "none", "outset", "ridge", "solid"]),
    ("border-style",            &["dashed", "dotted", "double", "groove", "hidden", "inset", "none", "outset", "ridge", "solid"]),
    ("border-top-style",        &["dashed", "dotted", "double", "groove", "hidden", "inset", "none", "outset", "ridge", "solid"]),
    ("box-decoration-break",    &["clone", "slice"]),
    ("box-sizing",              &["border-box", "content-box"]),
    ("break-after",             &["all", "always", "auto", "avoid", "avoid-column", "avoid-page", "avoid-region", "column", "left", "page", "recto", "region", "right", "verso"]),
    ("break-before",            &["all", "always", "auto", "avoid", "avoid-column", "avoid-page", "avoid-region", "column", "left", "page", "recto", "region", "right", "verso"]),
    ("break-inside",            &["auto", "avoid", "avoid-column", "avoid-page", "avoid-region"]),
    ("caption-side",            &["bottom", "top"]),
    ("clear",                   &["both", "inline-end", "inline-start", "left", "none", "right"]),
    ("color-scheme",            &["dark", "light", "normal"]),
    ("column-fill",             &["auto", "balance"]),
    ("column-span",             &["all", "none"]),
    ("contain",                 &["content", "inline-size", "layout", "none", "paint", "size", "strict", "style"]),
    ("container-type",          &["inline-size", "normal", "size"]),
    ("content-visibility",      &["auto", "hidden", "visible"]),
    ("cursor",                  &["alias", "all-scroll", "auto", "cell", "col-resize", "context-menu", "copy", "crosshair", "default", "e-resize", "ew-resize", "grab", "grabbing", "help", "move", "n-resize", "ne-resize", "nesw-resize", "no-drop", "none", "not-allowed", "ns-resize", "nw-resize", "nwse-resize", "pointer", "progress", "row-resize", "s-resize", "se-resize", "sw-resize", "text", "vertical-text", "w-resize", "wait", "zoom-in", "zoom-out"]),
    ("direction",               &["ltr", "rtl"]),
    ("display",                 &["block", "contents", "flex", "flow-root", "grid", "inline", "inline-block", "inline-flex", "inline-grid", "inline-table", "list-item", "none", "table", "table-caption", "table-cell", "table-column", "table-column-group", "table-footer-group", "table-header-group", "table-row", "table-row-group"]),
    ("empty-cells",             &["hide", "show"]),
    ("flex-direction",          &["column", "column-reverse", "row", "row-reverse"]),
    ("flex-wrap",               &["nowrap", "wrap", "wrap-reverse"]),
    ("float",                   &["inline-end", "inline-start", "left", "none", "right"]),
    ("font-kerning",            &["auto", "none", "normal"]),
    ("font-optical-sizing",     &["auto", "none"]),
    ("font-stretch",            &["condensed", "expanded", "extra-condensed", "extra-expanded", "normal", "semi-condensed", "semi-expanded", "ultra-condensed", "ultra-expanded"]),
    ("font-style",              &["italic", "normal", "oblique"]),
    ("font-variant",            &["normal", "small-caps"]),
    ("font-variant-caps",       &["all-petite-caps", "all-small-caps", "normal", "petite-caps", "small-caps", "titling-caps", "unicase"]),
    ("font-variant-ligatures",  &["common-ligatures", "contextual", "discretionary-ligatures", "historical-ligatures", "no-common-ligatures", "no-contextual", "no-discretionary-ligatures", "no-historical-ligatures", "none", "normal"]),
    ("font-variant-numeric",    &["diagonal-fractions", "lining-nums", "normal", "oldstyle-nums", "ordinal", "proportional-nums", "slashed-zero", "stacked-fractions", "tabular-nums"]),
    ("font-weight",             &["bold", "bolder", "lighter", "normal"]),
    ("forced-color-adjust",     &["auto", "none"]),
    ("grid-auto-flow",          &["column", "dense", "row"]),
    ("height",                  &["auto", "fit-content", "max-content", "min-content"]),
    ("hyphens",                 &["auto", "manual", "none"]),
    ("image-rendering",         &["auto", "crisp-edges", "pixelated"]),
    ("isolation",               &["auto", "isolate"]),
    ("justify-content",         &["center", "end", "flex-end", "flex-start", "left", "normal", "right", "space-around", "space-between", "space-evenly", "start", "stretch"]),
    ("justify-items",           &["baseline", "center", "end", "flex-end", "flex-start", "left", "normal", "right", "self-end", "self-start", "start", "stretch"]),
    ("justify-self",            &["auto", "baseline", "center", "end", "flex-end", "flex-start", "left", "normal", "right", "self-end", "self-start", "start", "stretch"]),
    ("list-style-position",     &["inside", "outside"]),
    ("list-style-type",         &["circle", "decimal", "decimal-leading-zero", "disc", "georgian", "lower-alpha", "lower-latin", "lower-roman", "none", "square", "upper-alpha", "upper-latin", "upper-roman"]),
    ("max-height",              &["fit-content", "max-content", "min-content", "none"]),
    ("max-width",               &["fit-content", "max-content", "min-content", "none"]),
    ("min-height",              &["auto", "fit-content", "max-content", "min-content"]),
    ("min-width",               &["auto", "fit-content", "max-content", "min-content"]),
    ("mix-blend-mode",          &["color", "color-burn", "color-dodge", "darken", "difference", "exclusion", "hard-light", "hue", "lighten", "luminosity", "multiply", "normal", "overlay", "saturation", "screen", "soft-light"]),
    ("object-fit",              &["contain", "cover", "fill", "none", "scale-down"]),
    ("outline-style",           &["auto", "dashed", "dotted", "double", "groove", "inset", "none", "outset", "ridge", "solid"]),
    ("overflow",                &["auto", "clip", "hidden", "scroll", "visible"]),
    ("overflow-anchor",         &["auto", "none"]),
    ("overflow-wrap",           &["anywhere", "break-word", "normal"]),
    ("overflow-x",              &["auto", "clip", "hidden", "scroll", "visible"]),
    ("overflow-y",              &["auto", "clip", "hidden", "scroll", "visible"]),
    ("overscroll-behavior",     &["auto", "contain", "none"]),
    ("overscroll-behavior-x",   &["auto", "contain", "none"]),
    ("overscroll-behavior-y",   &["auto", "contain", "none"]),
    ("page-break-after",        &["always", "auto", "avoid", "left", "right"]),
    ("page-break-before",       &["always", "auto", "avoid", "left", "right"]),
    ("page-break-inside",       &["auto", "avoid"]),
    ("place-content",           &["center", "end", "flex-end", "flex-start", "normal", "space-around", "space-between", "space-evenly", "start", "stretch"]),
    ("place-items",             &["baseline", "center", "end", "flex-end", "flex-start", "normal", "start", "stretch"]),
    ("place-self",              &["auto", "baseline", "center", "end", "flex-end", "flex-start", "normal", "start", "stretch"]),
    ("pointer-events",          &["all", "auto", "fill", "none", "painted", "stroke", "visible", "visibleFill", "visiblePainted", "visibleStroke"]),
    ("position",                &["absolute", "fixed", "relative", "static", "sticky"]),
    ("print-color-adjust",      &["economy", "exact"]),
    ("resize",                  &["block", "both", "horizontal", "inline", "none", "vertical"]),
    ("scroll-behavior",         &["auto", "smooth"]),
    ("scroll-snap-align",       &["center", "end", "none", "start"]),
    ("scroll-snap-stop",        &["always", "normal"]),
    ("scroll-snap-type",        &["block", "both", "inline", "mandatory", "none", "proximity", "x", "y"]),
    ("scrollbar-gutter",        &["auto", "stable"]),
    ("scrollbar-width",         &["auto", "none", "thin"]),
    ("table-layout",            &["auto", "fixed"]),
    ("text-align",              &["center", "end", "justify", "left", "right", "start"]),
    ("text-align-last",         &["auto", "center", "end", "justify", "left", "right", "start"]),
    ("text-decoration-line",    &["line-through", "none", "overline", "underline"]),
    ("text-decoration-style",   &["dashed", "dotted", "double", "solid", "wavy"]),
    ("text-overflow",           &["clip", "ellipsis"]),
    ("text-rendering",          &["auto", "geometricPrecision", "optimizeLegibility", "optimizeSpeed"]),
    ("text-transform",          &["capitalize", "full-width", "lowercase", "none", "uppercase"]),
    ("text-wrap",               &["balance", "nowrap", "pretty", "stable", "wrap"]),
    ("touch-action",            &["auto", "manipulation", "none", "pan-down", "pan-left", "pan-right", "pan-up", "pan-x", "pan-y", "pinch-zoom"]),
    ("transform-style",         &["flat", "preserve-3d"]),
    ("transition-timing-function", &["ease", "ease-in", "ease-in-out", "ease-out", "linear", "step-end", "step-start"]),
    ("unicode-bidi",            &["bidi-override", "embed", "isolate", "isolate-override", "normal", "plaintext"]),
    ("user-select",             &["all", "auto", "contain", "none", "text"]),
    ("vertical-align",          &["baseline", "bottom", "middle", "sub", "super", "text-bottom", "text-top", "top"]),
    ("visibility",              &["collapse", "hidden", "visible"]),
    ("white-space",             &["break-spaces", "normal", "nowrap", "pre", "pre-line", "pre-wrap"]),
    ("width",                   &["auto", "fit-content", "max-content", "min-content"]),
    ("word-break",              &["break-all", "break-word", "keep-all", "normal"]),
    ("word-wrap",               &["break-word", "normal"]),
    ("writing-mode",            &["horizontal-tb", "sideways-lr", "sideways-rl", "vertical-lr", "vertical-rl"]),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_known_property() {
        let values = values_for_property("display");
        assert!(values.contains(&"flex"));
        assert!(values.contains(&"grid"));
        assert!(values.contains(&"none"));
    }

    #[test]
    fn lookup_unknown_property() {
        assert!(values_for_property("made-up-property").is_empty());
    }

    #[test]
    fn lookup_case_insensitive() {
        let values = values_for_property("Display");
        assert!(values.contains(&"flex"));
    }

    #[test]
    fn data_is_sorted() {
        for window in CSS_PROPERTY_VALUES.windows(2) {
            assert!(
                window[0].0 < window[1].0,
                "CSS_PROPERTY_VALUES not sorted: {:?} >= {:?}",
                window[0].0,
                window[1].0,
            );
        }
    }
}
