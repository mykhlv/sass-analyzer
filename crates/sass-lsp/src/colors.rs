use dashmap::DashMap;
use sass_parser::syntax::SyntaxNode;
use sass_parser::syntax_kind::SyntaxKind;
use tower_lsp_server::ls_types::{
    Color, ColorInformation, ColorPresentation, ColorPresentationParams, DocumentColorParams, Uri,
};

use crate::DocumentState;
use crate::ast_helpers::first_ident_token;
use crate::convert::text_range_to_lsp;

// ---------------------------------------------------------------------------
// Public handlers
// ---------------------------------------------------------------------------

pub(crate) fn handle_document_color(
    documents: &DashMap<Uri, DocumentState>,
    params: DocumentColorParams,
) -> Vec<ColorInformation> {
    let uri = params.text_document.uri;
    let Some(doc) = documents.get(&uri) else {
        return Vec::new();
    };
    let root = SyntaxNode::new_root(doc.green.clone());
    let mut colors = Vec::new();
    collect_colors(&root, &doc.text, &doc.line_index, &mut colors);
    colors
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(crate) fn handle_color_presentation(
    params: &ColorPresentationParams,
) -> Vec<ColorPresentation> {
    let color = &params.color;
    let red = (color.red * 255.0).clamp(0.0, 255.0).round() as u8;
    let green = (color.green * 255.0).clamp(0.0, 255.0).round() as u8;
    let blue = (color.blue * 255.0).clamp(0.0, 255.0).round() as u8;
    let alpha = color.alpha.clamp(0.0, 1.0);

    let mut presentations = Vec::with_capacity(3);

    // Hex
    let is_opaque = (alpha - 1.0).abs() < f32::EPSILON;
    if is_opaque {
        if red >> 4 == red & 0xF && green >> 4 == green & 0xF && blue >> 4 == blue & 0xF {
            presentations.push(ColorPresentation {
                label: format!("#{:x}{:x}{:x}", red & 0xF, green & 0xF, blue & 0xF),
                ..ColorPresentation::default()
            });
        }
        presentations.push(ColorPresentation {
            label: format!("#{red:02x}{green:02x}{blue:02x}"),
            ..ColorPresentation::default()
        });
    } else {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let alpha8 = (alpha * 255.0).round() as u8;
        presentations.push(ColorPresentation {
            label: format!("#{red:02x}{green:02x}{blue:02x}{alpha8:02x}"),
            ..ColorPresentation::default()
        });
    }

    // rgb/rgba
    if is_opaque {
        presentations.push(ColorPresentation {
            label: format!("rgb({red}, {green}, {blue})"),
            ..ColorPresentation::default()
        });
    } else {
        presentations.push(ColorPresentation {
            label: format!("rgba({red}, {green}, {blue}, {})", format_alpha(alpha)),
            ..ColorPresentation::default()
        });
    }

    // hsl/hsla
    let (hue, sat, lig) = rgb_to_hsl(color.red, color.green, color.blue);
    #[allow(clippy::cast_possible_truncation)]
    let hue_round = hue.round() as i32;
    #[allow(clippy::cast_possible_truncation)]
    let sat_round = (sat * 100.0).round() as i32;
    #[allow(clippy::cast_possible_truncation)]
    let lig_round = (lig * 100.0).round() as i32;
    if is_opaque {
        presentations.push(ColorPresentation {
            label: format!("hsl({hue_round}, {sat_round}%, {lig_round}%)"),
            ..ColorPresentation::default()
        });
    } else {
        presentations.push(ColorPresentation {
            label: format!(
                "hsla({hue_round}, {sat_round}%, {lig_round}%, {})",
                format_alpha(alpha)
            ),
            ..ColorPresentation::default()
        });
    }

    presentations
}

// ---------------------------------------------------------------------------
// Color collection from CST
// ---------------------------------------------------------------------------

fn collect_colors(
    root: &SyntaxNode,
    source: &str,
    line_index: &sass_parser::line_index::LineIndex,
    out: &mut Vec<ColorInformation>,
) {
    for node in root.descendants() {
        match node.kind() {
            SyntaxKind::COLOR_LITERAL => {
                let text = node.text().to_string();
                let trimmed = text.trim_start();
                if let Some(color) = parse_hex_color(trimmed) {
                    out.push(ColorInformation {
                        range: text_range_to_lsp(content_range(&node), line_index, source),
                        color,
                    });
                }
            }
            SyntaxKind::FUNCTION_CALL => {
                if let Some(name_tok) = first_ident_token(&node) {
                    let name = name_tok.text();
                    if let Some(color) = parse_color_function(name, &node) {
                        out.push(ColorInformation {
                            range: text_range_to_lsp(content_range(&node), line_index, source),
                            color,
                        });
                    }
                }
            }
            _ => {
                if is_value_context(node.kind()) {
                    collect_named_colors_in(&node, source, line_index, out);
                }
            }
        }
    }
}

/// Compute node range excluding leading whitespace trivia.
fn content_range(node: &SyntaxNode) -> sass_parser::text_range::TextRange {
    let range = node.text_range();
    // Find first non-whitespace child to skip leading trivia
    for child in node.children_with_tokens() {
        if let Some(token) = child.into_token() {
            if token.kind() != SyntaxKind::WHITESPACE {
                return sass_parser::text_range::TextRange::new(
                    token.text_range().start(),
                    range.end(),
                );
            }
        } else {
            // First child is a node — no leading trivia to skip
            break;
        }
    }
    range
}

fn is_value_context(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::DECLARATION
            | SyntaxKind::CUSTOM_PROPERTY_DECL
            | SyntaxKind::VARIABLE_DECL
            | SyntaxKind::RETURN_RULE
            | SyntaxKind::MAP_ENTRY
    )
}

fn collect_named_colors_in(
    node: &SyntaxNode,
    source: &str,
    line_index: &sass_parser::line_index::LineIndex,
    out: &mut Vec<ColorInformation>,
) {
    for child in node.children_with_tokens() {
        if let Some(token) = child.into_token() {
            if token.kind() == SyntaxKind::IDENT {
                let text = token.text();
                if text.eq_ignore_ascii_case("transparent") {
                    out.push(ColorInformation {
                        range: text_range_to_lsp(token.text_range(), line_index, source),
                        color: Color {
                            red: 0.0,
                            green: 0.0,
                            blue: 0.0,
                            alpha: 0.0,
                        },
                    });
                } else if let Some(rgb) = lookup_named_color(text) {
                    out.push(ColorInformation {
                        range: text_range_to_lsp(token.text_range(), line_index, source),
                        color: Color {
                            red: f32::from(rgb[0]) / 255.0,
                            green: f32::from(rgb[1]) / 255.0,
                            blue: f32::from(rgb[2]) / 255.0,
                            alpha: 1.0,
                        },
                    });
                }
            }
        }
    }
    // Recurse into child nodes that are expressions (not selectors/property names).
    // Skip children that are themselves value-context nodes — the outer `descendants()`
    // loop in `collect_colors` will handle those, avoiding duplicate results.
    for child in node.children() {
        if !matches!(
            child.kind(),
            SyntaxKind::SELECTOR_LIST
                | SyntaxKind::SELECTOR
                | SyntaxKind::SIMPLE_SELECTOR
                | SyntaxKind::PROPERTY
        ) && !is_value_context(child.kind())
        {
            collect_named_colors_in(&child, source, line_index, out);
        }
    }
}

// ---------------------------------------------------------------------------
// Hex color parsing
// ---------------------------------------------------------------------------

#[allow(clippy::many_single_char_names)]
fn parse_hex_color(text: &str) -> Option<Color> {
    let hex = text.strip_prefix('#')?;
    let (red, green, blue, alpha) = match hex.len() {
        3 => {
            let r = parse_hex_digit(hex.as_bytes()[0])?;
            let g = parse_hex_digit(hex.as_bytes()[1])?;
            let b = parse_hex_digit(hex.as_bytes()[2])?;
            ((r << 4) | r, (g << 4) | g, (b << 4) | b, 255u8)
        }
        4 => {
            let r = parse_hex_digit(hex.as_bytes()[0])?;
            let g = parse_hex_digit(hex.as_bytes()[1])?;
            let b = parse_hex_digit(hex.as_bytes()[2])?;
            let a = parse_hex_digit(hex.as_bytes()[3])?;
            ((r << 4) | r, (g << 4) | g, (b << 4) | b, (a << 4) | a)
        }
        6 => {
            let r = parse_hex_pair(hex, 0)?;
            let g = parse_hex_pair(hex, 2)?;
            let b = parse_hex_pair(hex, 4)?;
            (r, g, b, 255u8)
        }
        8 => {
            let r = parse_hex_pair(hex, 0)?;
            let g = parse_hex_pair(hex, 2)?;
            let b = parse_hex_pair(hex, 4)?;
            let a = parse_hex_pair(hex, 6)?;
            (r, g, b, a)
        }
        _ => return None,
    };
    Some(Color {
        red: f32::from(red) / 255.0,
        green: f32::from(green) / 255.0,
        blue: f32::from(blue) / 255.0,
        alpha: f32::from(alpha) / 255.0,
    })
}

fn parse_hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn parse_hex_pair(hex: &str, offset: usize) -> Option<u8> {
    let hi = parse_hex_digit(hex.as_bytes()[offset])?;
    let lo = parse_hex_digit(hex.as_bytes()[offset + 1])?;
    Some((hi << 4) | lo)
}

// ---------------------------------------------------------------------------
// Color function parsing (rgb/rgba/hsl/hsla)
// ---------------------------------------------------------------------------

#[allow(clippy::cast_possible_truncation)]
fn parse_color_function(name: &str, node: &SyntaxNode) -> Option<Color> {
    let name_lower = name.to_ascii_lowercase();
    let is_hsl = matches!(name_lower.as_str(), "hsl" | "hsla");
    let is_color_fn = is_hsl || matches!(name_lower.as_str(), "rgb" | "rgba");
    if !is_color_fn {
        return None;
    }

    let args = extract_function_args(node)?;

    let channels = match args.len() {
        3 | 4 => {
            if is_hsl {
                hsl_to_rgb(args[0], args[1], args[2])
            } else {
                (args[0] / 255.0, args[1] / 255.0, args[2] / 255.0)
            }
        }
        _ => return None,
    };

    let alpha = if args.len() == 4 { args[3] } else { 1.0 };

    Some(Color {
        red: channels.0.clamp(0.0, 1.0) as f32,
        green: channels.1.clamp(0.0, 1.0) as f32,
        blue: channels.2.clamp(0.0, 1.0) as f32,
        alpha: alpha.clamp(0.0, 1.0) as f32,
    })
}

fn extract_function_args(node: &SyntaxNode) -> Option<Vec<f64>> {
    let arg_list = node.children().find(|c| c.kind() == SyntaxKind::ARG_LIST)?;

    let arg_nodes: Vec<_> = arg_list
        .children()
        .filter(|c| c.kind() == SyntaxKind::ARG)
        .collect();

    if arg_nodes.len() >= 3 {
        return extract_comma_args(&arg_nodes);
    }

    if arg_nodes.len() == 1 {
        return extract_space_args(&arg_nodes[0]);
    }

    None
}

fn extract_comma_args(args: &[SyntaxNode]) -> Option<Vec<f64>> {
    let mut values = Vec::with_capacity(args.len());
    for arg in args {
        values.push(extract_single_numeric_value(arg)?);
    }
    Some(values)
}

fn extract_single_numeric_value(arg: &SyntaxNode) -> Option<f64> {
    for child in arg.descendants() {
        match child.kind() {
            SyntaxKind::NUMBER_LITERAL => {
                let text = child.text().to_string();
                return text.trim().parse::<f64>().ok();
            }
            SyntaxKind::DIMENSION => {
                return parse_dimension(&child);
            }
            SyntaxKind::VARIABLE_REF | SyntaxKind::FUNCTION_CALL => return None,
            _ => {}
        }
    }
    None
}

fn parse_dimension(node: &SyntaxNode) -> Option<f64> {
    let mut number = None;
    let mut unit = String::new();
    for child in node.children_with_tokens() {
        if let Some(token) = child.into_token() {
            match token.kind() {
                SyntaxKind::NUMBER => number = Some(token.text().parse::<f64>().ok()?),
                SyntaxKind::IDENT => unit = token.text().to_ascii_lowercase(),
                SyntaxKind::PERCENT => "%".clone_into(&mut unit),
                _ => {}
            }
        }
    }
    let val = number?;
    match unit.as_str() {
        "%" => Some(val / 100.0),
        "deg" => Some(val),
        "" => Some(val),
        _ => None,
    }
}

fn extract_space_args(arg: &SyntaxNode) -> Option<Vec<f64>> {
    let mut values = Vec::new();
    let mut has_slash = false;

    for item in arg.children_with_tokens() {
        match item {
            rowan::NodeOrToken::Token(tok) if tok.kind() == SyntaxKind::SLASH => {
                has_slash = true;
            }
            rowan::NodeOrToken::Node(node) => match node.kind() {
                SyntaxKind::NUMBER_LITERAL => {
                    let text = node.text().to_string();
                    values.push(text.trim().parse::<f64>().ok()?);
                }
                SyntaxKind::DIMENSION => {
                    values.push(parse_dimension(&node)?);
                }
                SyntaxKind::VARIABLE_REF | SyntaxKind::FUNCTION_CALL => return None,
                _ => {}
            },
            rowan::NodeOrToken::Token(_) => {}
        }
    }

    let expected = if has_slash { 4 } else { 3 };
    if values.len() == expected {
        Some(values)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Color space conversions
// ---------------------------------------------------------------------------

#[allow(clippy::many_single_char_names)]
fn hsl_to_rgb(hue: f64, sat: f64, lig: f64) -> (f64, f64, f64) {
    let hue = ((hue % 360.0) + 360.0) % 360.0;
    let chroma = (1.0 - (2.0 * lig - 1.0).abs()) * sat;
    let secondary = chroma * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
    let match_val = lig - chroma / 2.0;

    let (r1, g1, b1) = if hue < 60.0 {
        (chroma, secondary, 0.0)
    } else if hue < 120.0 {
        (secondary, chroma, 0.0)
    } else if hue < 180.0 {
        (0.0, chroma, secondary)
    } else if hue < 240.0 {
        (0.0, secondary, chroma)
    } else if hue < 300.0 {
        (secondary, 0.0, chroma)
    } else {
        (chroma, 0.0, secondary)
    };

    (
        (r1 + match_val).clamp(0.0, 1.0),
        (g1 + match_val).clamp(0.0, 1.0),
        (b1 + match_val).clamp(0.0, 1.0),
    )
}

#[allow(clippy::cast_possible_truncation)]
fn rgb_to_hsl(red: f32, green: f32, blue: f32) -> (f32, f32, f32) {
    let rf = f64::from(red);
    let gf = f64::from(green);
    let bf = f64::from(blue);
    let max = rf.max(gf).max(bf);
    let min = rf.min(gf).min(bf);
    let lig = (max + min) / 2.0;

    if (max - min).abs() < f64::EPSILON {
        return (0.0, 0.0, lig as f32);
    }

    let delta = max - min;
    let sat = if lig > 0.5 {
        delta / (2.0 - max - min)
    } else {
        delta / (max + min)
    };

    #[allow(clippy::float_cmp)]
    let hue = if max == rf {
        ((gf - bf) / delta) % 6.0
    } else if max == gf {
        (bf - rf) / delta + 2.0
    } else {
        (rf - gf) / delta + 4.0
    };
    let hue = ((hue * 60.0) + 360.0) % 360.0;

    (hue as f32, sat as f32, lig as f32)
}

fn format_alpha(alpha: f32) -> String {
    if (alpha * 100.0).fract().abs() < 0.01 {
        format!("{}", (alpha * 100.0).round() / 100.0)
    } else {
        format!("{alpha:.2}")
    }
}

// ---------------------------------------------------------------------------
// Named CSS colors (148 entries, sorted for binary search)
// ---------------------------------------------------------------------------

fn lookup_named_color(name: &str) -> Option<[u8; 3]> {
    let lower = name.to_ascii_lowercase();
    NAMED_COLORS
        .binary_search_by_key(&&*lower, |&(n, _)| n)
        .ok()
        .map(|i| NAMED_COLORS[i].1)
}

#[rustfmt::skip]
static NAMED_COLORS: &[(&str, [u8; 3])] = &[
    ("aliceblue",            [240, 248, 255]),
    ("antiquewhite",         [250, 235, 215]),
    ("aqua",                 [  0, 255, 255]),
    ("aquamarine",           [127, 255, 212]),
    ("azure",                [240, 255, 255]),
    ("beige",                [245, 245, 220]),
    ("bisque",               [255, 228, 196]),
    ("black",                [  0,   0,   0]),
    ("blanchedalmond",       [255, 235, 205]),
    ("blue",                 [  0,   0, 255]),
    ("blueviolet",           [138,  43, 226]),
    ("brown",                [165,  42,  42]),
    ("burlywood",            [222, 184, 135]),
    ("cadetblue",            [ 95, 158, 160]),
    ("chartreuse",           [127, 255,   0]),
    ("chocolate",            [210, 105,  30]),
    ("coral",                [255, 127,  80]),
    ("cornflowerblue",       [100, 149, 237]),
    ("cornsilk",             [255, 248, 220]),
    ("crimson",              [220,  20,  60]),
    ("cyan",                 [  0, 255, 255]),
    ("darkblue",             [  0,   0, 139]),
    ("darkcyan",             [  0, 139, 139]),
    ("darkgoldenrod",        [184, 134,  11]),
    ("darkgray",             [169, 169, 169]),
    ("darkgreen",            [  0, 100,   0]),
    ("darkgrey",             [169, 169, 169]),
    ("darkkhaki",            [189, 183, 107]),
    ("darkmagenta",          [139,   0, 139]),
    ("darkolivegreen",       [ 85, 107,  47]),
    ("darkorange",           [255, 140,   0]),
    ("darkorchid",           [153,  50, 204]),
    ("darkred",              [139,   0,   0]),
    ("darksalmon",           [233, 150, 122]),
    ("darkseagreen",         [143, 188, 143]),
    ("darkslateblue",        [ 72,  61, 139]),
    ("darkslategray",        [ 47,  79,  79]),
    ("darkslategrey",        [ 47,  79,  79]),
    ("darkturquoise",        [  0, 206, 209]),
    ("darkviolet",           [148,   0, 211]),
    ("deeppink",             [255,  20, 147]),
    ("deepskyblue",          [  0, 191, 255]),
    ("dimgray",              [105, 105, 105]),
    ("dimgrey",              [105, 105, 105]),
    ("dodgerblue",           [ 30, 144, 255]),
    ("firebrick",            [178,  34,  34]),
    ("floralwhite",          [255, 250, 240]),
    ("forestgreen",          [ 34, 139,  34]),
    ("fuchsia",              [255,   0, 255]),
    ("gainsboro",            [220, 220, 220]),
    ("ghostwhite",           [248, 248, 255]),
    ("gold",                 [255, 215,   0]),
    ("goldenrod",            [218, 165,  32]),
    ("gray",                 [128, 128, 128]),
    ("green",                [  0, 128,   0]),
    ("greenyellow",          [173, 255,  47]),
    ("grey",                 [128, 128, 128]),
    ("honeydew",             [240, 255, 240]),
    ("hotpink",              [255, 105, 180]),
    ("indianred",            [205,  92,  92]),
    ("indigo",               [ 75,   0, 130]),
    ("ivory",                [255, 255, 240]),
    ("khaki",                [240, 230, 140]),
    ("lavender",             [230, 230, 250]),
    ("lavenderblush",        [255, 240, 245]),
    ("lawngreen",            [124, 252,   0]),
    ("lemonchiffon",         [255, 250, 205]),
    ("lightblue",            [173, 216, 230]),
    ("lightcoral",           [240, 128, 128]),
    ("lightcyan",            [224, 255, 255]),
    ("lightgoldenrodyellow", [250, 250, 210]),
    ("lightgray",            [211, 211, 211]),
    ("lightgreen",           [144, 238, 144]),
    ("lightgrey",            [211, 211, 211]),
    ("lightpink",            [255, 182, 193]),
    ("lightsalmon",          [255, 160, 122]),
    ("lightseagreen",        [ 32, 178, 170]),
    ("lightskyblue",         [135, 206, 250]),
    ("lightslategray",       [119, 136, 153]),
    ("lightslategrey",       [119, 136, 153]),
    ("lightsteelblue",       [176, 196, 222]),
    ("lightyellow",          [255, 255, 224]),
    ("lime",                 [  0, 255,   0]),
    ("limegreen",            [ 50, 205,  50]),
    ("linen",                [250, 240, 230]),
    ("magenta",              [255,   0, 255]),
    ("maroon",               [128,   0,   0]),
    ("mediumaquamarine",     [102, 205, 170]),
    ("mediumblue",           [  0,   0, 205]),
    ("mediumorchid",         [186,  85, 211]),
    ("mediumpurple",         [147, 111, 219]),
    ("mediumseagreen",       [ 60, 179, 113]),
    ("mediumslateblue",      [123, 104, 238]),
    ("mediumspringgreen",    [  0, 250, 154]),
    ("mediumturquoise",      [ 72, 209, 204]),
    ("mediumvioletred",      [199,  21, 133]),
    ("midnightblue",         [ 25,  25, 112]),
    ("mintcream",            [245, 255, 250]),
    ("mistyrose",            [255, 228, 225]),
    ("moccasin",             [255, 228, 181]),
    ("navajowhite",          [255, 222, 173]),
    ("navy",                 [  0,   0, 128]),
    ("oldlace",              [253, 245, 230]),
    ("olive",                [128, 128,   0]),
    ("olivedrab",            [107, 142,  35]),
    ("orange",               [255, 165,   0]),
    ("orangered",            [255,  69,   0]),
    ("orchid",               [218, 112, 214]),
    ("palegoldenrod",        [238, 232, 170]),
    ("palegreen",            [152, 251, 152]),
    ("paleturquoise",        [175, 238, 238]),
    ("palevioletred",        [219, 112, 147]),
    ("papayawhip",           [255, 239, 213]),
    ("peachpuff",            [255, 218, 185]),
    ("peru",                 [205, 133,  63]),
    ("pink",                 [255, 192, 203]),
    ("plum",                 [221, 160, 221]),
    ("powderblue",           [176, 224, 230]),
    ("purple",               [128,   0, 128]),
    ("rebeccapurple",        [102,  51, 153]),
    ("red",                  [255,   0,   0]),
    ("rosybrown",            [188, 143, 143]),
    ("royalblue",            [ 65, 105, 225]),
    ("saddlebrown",          [139,  69,  19]),
    ("salmon",               [250, 128, 114]),
    ("sandybrown",           [244, 164,  96]),
    ("seagreen",             [ 46, 139,  87]),
    ("seashell",             [255, 245, 238]),
    ("sienna",               [160,  82,  45]),
    ("silver",               [192, 192, 192]),
    ("skyblue",              [135, 206, 235]),
    ("slateblue",            [106,  90, 205]),
    ("slategray",            [112, 128, 144]),
    ("slategrey",            [112, 128, 144]),
    ("snow",                 [255, 250, 250]),
    ("springgreen",          [  0, 255, 127]),
    ("steelblue",            [ 70, 130, 180]),
    ("tan",                  [210, 180, 140]),
    ("teal",                 [  0, 128, 128]),
    ("thistle",              [216, 191, 216]),
    ("tomato",               [255,  99,  71]),
    ("turquoise",            [ 64, 224, 208]),
    ("violet",               [238, 130, 238]),
    ("wheat",                [245, 222, 179]),
    ("white",                [255, 255, 255]),
    ("whitesmoke",           [245, 245, 245]),
    ("yellow",               [255, 255,   0]),
    ("yellowgreen",          [154, 205,  50]),
];

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use tower_lsp_server::ls_types::{PartialResultParams, WorkDoneProgressParams};

    use super::*;

    fn make_presentation_params(
        red: f32,
        green: f32,
        blue: f32,
        alpha: f32,
    ) -> ColorPresentationParams {
        ColorPresentationParams {
            text_document: tower_lsp_server::ls_types::TextDocumentIdentifier {
                uri: Uri::from_str("file:///test.scss").unwrap(),
            },
            color: Color {
                red,
                green,
                blue,
                alpha,
            },
            range: tower_lsp_server::ls_types::Range::default(),
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        }
    }

    fn assert_color_approx(
        color: Color,
        expected_r: f32,
        expected_g: f32,
        expected_b: f32,
        expected_a: f32,
    ) {
        let eps = 0.01;
        assert!(
            (color.red - expected_r).abs() < eps
                && (color.green - expected_g).abs() < eps
                && (color.blue - expected_b).abs() < eps
                && (color.alpha - expected_a).abs() < eps,
            "expected ({expected_r}, {expected_g}, {expected_b}, {expected_a}), got ({}, {}, {}, {})",
            color.red,
            color.green,
            color.blue,
            color.alpha
        );
    }

    // -- Hex parsing --

    #[test]
    fn hex_3_digit() {
        let color = parse_hex_color("#fff").unwrap();
        assert_color_approx(color, 1.0, 1.0, 1.0, 1.0);
    }

    #[test]
    fn hex_3_digit_color() {
        let color = parse_hex_color("#f00").unwrap();
        assert_color_approx(color, 1.0, 0.0, 0.0, 1.0);
    }

    #[test]
    fn hex_6_digit() {
        let color = parse_hex_color("#3498db").unwrap();
        assert_color_approx(color, 0.204, 0.596, 0.859, 1.0);
    }

    #[test]
    fn hex_4_digit_with_alpha() {
        let color = parse_hex_color("#f00a").unwrap();
        assert_color_approx(color, 1.0, 0.0, 0.0, 0.667);
    }

    #[test]
    fn hex_8_digit_with_alpha() {
        let color = parse_hex_color("#3498db80").unwrap();
        assert_color_approx(color, 0.204, 0.596, 0.859, 0.502);
    }

    #[test]
    fn hex_invalid_length() {
        assert!(parse_hex_color("#12345").is_none());
    }

    #[test]
    fn hex_invalid_char() {
        assert!(parse_hex_color("#xyz").is_none());
    }

    #[test]
    fn hex_uppercase() {
        let color = parse_hex_color("#FF0000").unwrap();
        assert_color_approx(color, 1.0, 0.0, 0.0, 1.0);
    }

    // -- Named colors --

    #[test]
    fn named_color_red() {
        let rgb = lookup_named_color("red").unwrap();
        assert_eq!(rgb, [255, 0, 0]);
    }

    #[test]
    fn named_color_case_insensitive() {
        let rgb = lookup_named_color("DarkSlateGray").unwrap();
        assert_eq!(rgb, [47, 79, 79]);
    }

    #[test]
    fn named_color_not_found() {
        assert!(lookup_named_color("foobar").is_none());
    }

    #[test]
    fn named_color_transparent_not_in_table() {
        assert!(lookup_named_color("transparent").is_none());
    }

    // -- HSL conversion --

    #[test]
    fn hsl_pure_red() {
        let (red, green, blue) = hsl_to_rgb(0.0, 1.0, 0.5);
        assert!((red - 1.0).abs() < 0.01);
        assert!(green.abs() < 0.01);
        assert!(blue.abs() < 0.01);
    }

    #[test]
    fn hsl_pure_green() {
        let (red, green, blue) = hsl_to_rgb(120.0, 1.0, 0.5);
        assert!(red.abs() < 0.01);
        assert!((green - 1.0).abs() < 0.01);
        assert!(blue.abs() < 0.01);
    }

    #[test]
    fn hsl_pure_blue() {
        let (red, green, blue) = hsl_to_rgb(240.0, 1.0, 0.5);
        assert!(red.abs() < 0.01);
        assert!(green.abs() < 0.01);
        assert!((blue - 1.0).abs() < 0.01);
    }

    #[test]
    fn hsl_roundtrip() {
        let (hue, sat, lig) = rgb_to_hsl(0.204, 0.596, 0.859);
        let (red, green, blue) = hsl_to_rgb(f64::from(hue), f64::from(sat), f64::from(lig));
        assert!((red - 0.204).abs() < 0.02);
        assert!((green - 0.596).abs() < 0.02);
        assert!((blue - 0.859).abs() < 0.02);
    }

    // -- Color presentation --

    #[test]
    fn presentation_opaque_red() {
        let params = make_presentation_params(1.0, 0.0, 0.0, 1.0);
        let presentations = handle_color_presentation(&params);
        let labels: Vec<&str> = presentations.iter().map(|p| p.label.as_str()).collect();
        assert!(labels.contains(&"#f00"));
        assert!(labels.contains(&"#ff0000"));
        assert!(labels.contains(&"rgb(255, 0, 0)"));
        assert!(labels.iter().any(|l| l.starts_with("hsl(")));
    }

    #[test]
    fn presentation_with_alpha() {
        let params = make_presentation_params(1.0, 0.0, 0.0, 0.5);
        let presentations = handle_color_presentation(&params);
        let labels: Vec<&str> = presentations.iter().map(|p| p.label.as_str()).collect();
        assert!(labels.iter().any(|l| l.starts_with("#ff0000")));
        assert!(labels.iter().any(|l| l.starts_with("rgba(")));
        assert!(labels.iter().any(|l| l.starts_with("hsla(")));
    }

    #[test]
    fn presentation_shorthand_eligible() {
        let params = make_presentation_params(1.0, 1.0, 1.0, 1.0);
        let presentations = handle_color_presentation(&params);
        let labels: Vec<&str> = presentations.iter().map(|p| p.label.as_str()).collect();
        assert!(labels.contains(&"#fff"));
        assert!(labels.contains(&"#ffffff"));
    }

    #[test]
    fn format_alpha_clean_values() {
        assert_eq!(format_alpha(0.5), "0.5");
        assert_eq!(format_alpha(0.1), "0.1");
        assert_eq!(format_alpha(1.0), "1");
        assert_eq!(format_alpha(0.0), "0");
    }

    #[test]
    fn named_color_table_is_sorted() {
        for window in NAMED_COLORS.windows(2) {
            assert!(
                window[0].0 < window[1].0,
                "named color table not sorted: {:?} >= {:?}",
                window[0].0,
                window[1].0
            );
        }
    }
}
