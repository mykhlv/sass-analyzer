use crate::symbols::{Symbol, SymbolKind};
use sass_parser::text_range::TextRange;

fn builtin_var(name: &str, doc: &str) -> Symbol {
    Symbol {
        name: name.into(),
        kind: SymbolKind::Variable,
        range: TextRange::empty(0.into()),
        selection_range: TextRange::empty(0.into()),
        params: None,
        value: None,
        doc: Some(doc.into()),
    }
}

fn builtin_fn(name: &str, params: &str, doc: &str) -> Symbol {
    Symbol {
        name: name.into(),
        kind: SymbolKind::Function,
        range: TextRange::empty(0.into()),
        selection_range: TextRange::empty(0.into()),
        params: Some(params.into()),
        value: None,
        doc: Some(doc.into()),
    }
}

fn builtin_mixin(name: &str, params: &str, doc: &str) -> Symbol {
    Symbol {
        name: name.into(),
        kind: SymbolKind::Mixin,
        range: TextRange::empty(0.into()),
        selection_range: TextRange::empty(0.into()),
        params: Some(params.into()),
        value: None,
        doc: Some(doc.into()),
    }
}

pub fn builtin_symbols(module: &str) -> Option<Vec<Symbol>> {
    match module {
        "math" => Some(math_symbols()),
        "color" => Some(color_symbols()),
        "list" => Some(list_symbols()),
        "map" => Some(map_symbols()),
        "string" => Some(string_symbols()),
        "meta" => Some(meta_symbols()),
        "selector" => Some(selector_symbols()),
        _ => None,
    }
}

pub fn builtin_uri(module: &str) -> String {
    format!("sass-builtin:///{module}")
}

pub fn is_builtin_uri(uri: &str) -> bool {
    uri.starts_with("sass-builtin:///")
}

pub fn builtin_name_from_uri(uri: &str) -> Option<&str> {
    uri.strip_prefix("sass-builtin:///")
}

fn math_symbols() -> Vec<Symbol> {
    vec![
        builtin_var("pi", "The value of pi."),
        builtin_var("e", "The value of e."),
        builtin_var(
            "epsilon",
            "The smallest representable distance between two numbers.",
        ),
        builtin_var("max-safe-integer", "The maximum safe integer."),
        builtin_var("min-safe-integer", "The minimum safe integer."),
        builtin_var("max-number", "The largest finite number representable."),
        builtin_var("min-number", "The smallest positive number representable."),
        builtin_var("infinity", "Positive infinity."),
        builtin_var("nan", "Not a Number."),
        builtin_fn("ceil", "$number", "Rounds up to the nearest whole number."),
        builtin_fn(
            "floor",
            "$number",
            "Rounds down to the nearest whole number.",
        ),
        builtin_fn("round", "$number", "Rounds to the nearest whole number."),
        builtin_fn("abs", "$number", "Returns the absolute value."),
        builtin_fn("max", "$numbers...", "Returns the highest value."),
        builtin_fn("min", "$numbers...", "Returns the lowest value."),
        builtin_fn(
            "clamp",
            "$min, $number, $max",
            "Clamps a number between min and max.",
        ),
        builtin_fn("sqrt", "$number", "Returns the square root."),
        builtin_fn(
            "pow",
            "$base, $exponent",
            "Raises base to the power of exponent.",
        ),
        builtin_fn("log", "$number, $base: null", "Returns the logarithm."),
        builtin_fn("hypot", "$numbers...", "Returns the hypotenuse."),
        builtin_fn("sin", "$number", "Returns the sine."),
        builtin_fn("cos", "$number", "Returns the cosine."),
        builtin_fn("tan", "$number", "Returns the tangent."),
        builtin_fn("asin", "$number", "Returns the arcsine."),
        builtin_fn("acos", "$number", "Returns the arccosine."),
        builtin_fn("atan", "$number", "Returns the arctangent."),
        builtin_fn("atan2", "$y, $x", "Returns the 2-argument arctangent."),
        builtin_fn(
            "compatible",
            "$number1, $number2",
            "Returns whether two numbers have compatible units.",
        ),
        builtin_fn(
            "is-unitless",
            "$number",
            "Returns whether a number has no units.",
        ),
        builtin_fn("unit", "$number", "Returns the unit(s) of a number."),
        builtin_fn(
            "percentage",
            "$number",
            "Converts a unitless number to a percentage.",
        ),
        builtin_fn("random", "$limit: null", "Returns a random number."),
        builtin_fn("div", "$number1, $number2", "Divides two numbers."),
    ]
}

fn color_symbols() -> Vec<Symbol> {
    vec![
        builtin_fn("red", "$color", "Returns the red channel."),
        builtin_fn("green", "$color", "Returns the green channel."),
        builtin_fn("blue", "$color", "Returns the blue channel."),
        builtin_fn("hue", "$color", "Returns the hue."),
        builtin_fn("saturation", "$color", "Returns the saturation."),
        builtin_fn("lightness", "$color", "Returns the lightness."),
        builtin_fn("alpha", "$color", "Returns the alpha channel."),
        builtin_fn(
            "adjust",
            "$color, $kwargs...",
            "Increases or decreases color properties.",
        ),
        builtin_fn(
            "scale",
            "$color, $kwargs...",
            "Fluidly scales color properties.",
        ),
        builtin_fn("change", "$color, $kwargs...", "Sets color properties."),
        builtin_fn(
            "mix",
            "$color1, $color2, $weight: 50%",
            "Mixes two colors together.",
        ),
        builtin_fn("complement", "$color", "Returns the complement."),
        builtin_fn("grayscale", "$color", "Returns a grayscale color."),
        builtin_fn(
            "invert",
            "$color, $weight: 100%",
            "Returns an inverted color.",
        ),
        builtin_fn(
            "ie-hex-str",
            "$color",
            "Returns an IE-compatible hex string.",
        ),
        builtin_fn(
            "hwb",
            "$hue, $whiteness, $blackness, $alpha: 1",
            "Creates a color from HWB.",
        ),
        builtin_fn("whiteness", "$color", "Returns the HWB whiteness."),
        builtin_fn("blackness", "$color", "Returns the HWB blackness."),
    ]
}

fn list_symbols() -> Vec<Symbol> {
    vec![
        builtin_fn(
            "append",
            "$list, $val, $separator: auto",
            "Adds a value to the end of a list.",
        ),
        builtin_fn(
            "index",
            "$list, $value",
            "Returns the index of a value in a list.",
        ),
        builtin_fn(
            "is-bracketed",
            "$list",
            "Returns whether a list has square brackets.",
        ),
        builtin_fn(
            "join",
            "$list1, $list2, $separator: auto, $bracketed: auto",
            "Combines two lists.",
        ),
        builtin_fn("length", "$list", "Returns the number of elements."),
        builtin_fn("separator", "$list", "Returns the separator of a list."),
        builtin_fn("nth", "$list, $n", "Returns the nth element."),
        builtin_fn("set-nth", "$list, $n, $value", "Replaces the nth element."),
        builtin_fn(
            "zip",
            "$lists...",
            "Combines lists into a single list of sub-lists.",
        ),
        builtin_fn("slash", "$elements...", "Returns a slash-separated list."),
    ]
}

fn map_symbols() -> Vec<Symbol> {
    vec![
        builtin_fn(
            "get",
            "$map, $key, $keys...",
            "Returns the value for a key.",
        ),
        builtin_fn(
            "has-key",
            "$map, $key, $keys...",
            "Returns whether a map has a key.",
        ),
        builtin_fn("keys", "$map", "Returns the keys in a map."),
        builtin_fn("values", "$map", "Returns the values in a map."),
        builtin_fn("merge", "$map1, $args...", "Merges maps together."),
        builtin_fn("remove", "$map, $keys...", "Removes keys from a map."),
        builtin_fn("deep-merge", "$map1, $map2", "Recursively merges two maps."),
        builtin_fn(
            "deep-remove",
            "$map, $key, $keys...",
            "Removes a key from nested maps.",
        ),
        builtin_fn("set", "$map, $args...", "Sets a value in a map."),
    ]
}

fn string_symbols() -> Vec<Symbol> {
    vec![
        builtin_fn("quote", "$string", "Adds quotes to a string."),
        builtin_fn("unquote", "$string", "Removes quotes from a string."),
        builtin_fn(
            "index",
            "$string, $substring",
            "Returns the index of a substring.",
        ),
        builtin_fn(
            "insert",
            "$string, $insert, $index",
            "Inserts text at a given index.",
        ),
        builtin_fn("length", "$string", "Returns the number of characters."),
        builtin_fn(
            "slice",
            "$string, $start-at, $end-at: -1",
            "Extracts a substring.",
        ),
        builtin_fn(
            "split",
            "$string, $separator, $limit: null",
            "Splits a string.",
        ),
        builtin_fn("to-lower-case", "$string", "Converts to lowercase."),
        builtin_fn("to-upper-case", "$string", "Converts to uppercase."),
        builtin_fn("unique-id", "", "Returns a unique CSS identifier."),
    ]
}

fn meta_symbols() -> Vec<Symbol> {
    vec![
        builtin_fn(
            "calc-args",
            "$calc",
            "Returns the arguments of a calculation.",
        ),
        builtin_fn("calc-name", "$calc", "Returns the name of a calculation."),
        builtin_fn(
            "call",
            "$function, $args...",
            "Calls a function by reference.",
        ),
        builtin_fn("content-exists", "", "Returns whether @content was passed."),
        builtin_fn(
            "feature-exists",
            "$feature",
            "Returns whether a feature exists.",
        ),
        builtin_fn(
            "function-exists",
            "$name, $module: null",
            "Returns whether a function exists.",
        ),
        builtin_fn(
            "get-function",
            "$name, $css: false, $module: null",
            "Returns a function reference.",
        ),
        builtin_fn(
            "global-variable-exists",
            "$name, $module: null",
            "Returns whether a global variable exists.",
        ),
        builtin_fn("inspect", "$value", "Returns a string representation."),
        builtin_fn("keywords", "$args", "Returns keyword arguments as a map."),
        builtin_fn(
            "mixin-exists",
            "$name, $module: null",
            "Returns whether a mixin exists.",
        ),
        builtin_fn(
            "module-functions",
            "$module",
            "Returns all functions in a module.",
        ),
        builtin_fn(
            "module-mixins",
            "$module",
            "Returns all mixins in a module.",
        ),
        builtin_fn(
            "module-variables",
            "$module",
            "Returns all variables in a module.",
        ),
        builtin_fn("type-of", "$value", "Returns the type of a value."),
        builtin_fn(
            "variable-exists",
            "$name",
            "Returns whether a variable exists.",
        ),
        builtin_mixin(
            "load-css",
            "$url, $with: null",
            "Dynamically loads a CSS module.",
        ),
        builtin_mixin("apply", "$mixin, $args...", "Dynamically includes a mixin."),
    ]
}

fn selector_symbols() -> Vec<Symbol> {
    vec![
        builtin_fn("append", "$selectors...", "Appends selectors."),
        builtin_fn(
            "extend",
            "$selector, $extendee, $extender",
            "Extends a selector.",
        ),
        builtin_fn(
            "is-superselector",
            "$super, $sub",
            "Returns whether one selector matches a superset.",
        ),
        builtin_fn("nest", "$selectors...", "Nests selectors."),
        builtin_fn("parse", "$selector", "Parses a selector string."),
        builtin_fn(
            "replace",
            "$selector, $original, $replacement",
            "Replaces a selector.",
        ),
        builtin_fn(
            "simple-selectors",
            "$selector",
            "Returns the simple selectors.",
        ),
        builtin_fn(
            "unify",
            "$selector1, $selector2",
            "Returns a selector that matches both inputs.",
        ),
    ]
}
