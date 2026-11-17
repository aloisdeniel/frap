// SPDX-License-Identifier: GPL-3.0-or-later
//! Path templates of render output paths (`{scene_name}`, `{resolution_x:####}`, …), Blender
//! 5.x `BKE_path_apply_template` (`blenkernel/intern/path_templates.cc`, GPL-2.0-or-later),
//! re-implemented from behaviour verified on Blender 5.2.4:
//!
//! - `{{` and `}}` are a literal brace; any other `}` is the error `Unescaped curly brace '}'.`;
//! - `{name}` / `{name:format}` is an expression. A `{` that reaches the end of the path or
//!   another `{` before its `}` is `Invalid or incomplete template expression '{…'.` (the text
//!   up to, not including, the next `{`), and parsing resumes at that `{`;
//! - the format is `#…` (integer, zero-padded to the number of `#`) or `#….#…` (fixed point:
//!   the integer part zero-padded to the `#` before the dot, as many decimals as `#` after it;
//!   at least one decimal). Anything else, including an empty format, is `Invalid format
//!   specifier in template expression '{…}'.`; the format is checked before the variable, and
//!   string variables take no format at all;
//! - an unknown variable is `Unknown variable referenced in template expression '{…}'.`
//!   (names are case-sensitive; `{}` and `{ x }` are unknown variables);
//! - every error of the path is collected, in order. The operator raises
//!   `RuntimeError: Error: Parse errors in path '<raw path>':` with one `- <error>` line each.
//!
//! The render output variables are `blend_name` (`Unsaved` before the first save), `fps`
//! (`fps / fps_base` as a float), `resolution_x` / `resolution_y` (after
//! `resolution_percentage`), `scene_name` and `camera_name` (made safe for file names).
//! `{fps}` without a format prints like Python's `repr(float)`: `24.0`, `29.970028570928392`.

/// A template variable's value.
#[derive(Clone, Debug, PartialEq)]
pub enum TemplateValue {
    /// A string, inserted as is.
    Str(String),
    /// An integer.
    Int(i64),
    /// A float.
    Float(f64),
}

/// The variables of a render output path.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TemplateVars {
    vars: Vec<(String, TemplateValue)>,
}

impl TemplateVars {
    /// No variables.
    pub fn new() -> TemplateVars {
        TemplateVars::default()
    }

    /// Adds (or replaces) a variable.
    pub fn with(mut self, name: &str, v: TemplateValue) -> TemplateVars {
        self.vars.retain(|(n, _)| n != name);
        self.vars.push((name.to_owned(), v));
        self
    }

    /// The render output variables (`BKE_build_template_variables_for_render_path`).
    ///
    /// `blend_name`: the file name of `bpy.data.filepath` without its extension (`None`:
    /// unsaved, `Unsaved`); `camera_name`: the scene camera object's name, when there is one.
    pub fn render(
        blend_name: Option<&str>,
        scene_name: &str,
        camera_name: Option<&str>,
        resolution: [i32; 2],
        fps: i32,
        fps_base: f32,
    ) -> TemplateVars {
        let mut v = TemplateVars::new()
            .with(
                "blend_name",
                TemplateValue::Str(make_safe_filename(blend_name.unwrap_or("Unsaved"))),
            )
            .with(
                "fps",
                TemplateValue::Float(f64::from(fps) / f64::from(fps_base)),
            )
            .with("resolution_x", TemplateValue::Int(i64::from(resolution[0])))
            .with("resolution_y", TemplateValue::Int(i64::from(resolution[1])))
            .with(
                "scene_name",
                TemplateValue::Str(make_safe_filename(scene_name)),
            );
        if let Some(c) = camera_name {
            v = v.with("camera_name", TemplateValue::Str(make_safe_filename(c)));
        }
        v
    }

    fn get(&self, name: &str) -> Option<&TemplateValue> {
        self.vars.iter().find(|(n, _)| n == name).map(|(_, v)| v)
    }
}

/// `BLI_path_make_safe_filename`: characters invalid in a file name on any platform (control
/// characters, `\ / : * ? " < > |`) become `_`; a name made only of dots becomes underscores.
pub fn make_safe_filename(name: &str) -> String {
    const INVALID: &[char] = &['\\', '/', ':', '*', '?', '"', '<', '>', '|'];
    let mut out: String = name
        .chars()
        .map(|c| {
            if (c as u32) < 32 || c as u32 == 127 || INVALID.contains(&c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    if !out.is_empty() && out.chars().all(|c| c == '.') {
        out = "_".repeat(out.len());
    }
    out
}

/// A parsed format specifier.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Format {
    /// `###`: integer, zero-padded to this many digits.
    Int(usize),
    /// `##.###`: integer digits, decimals.
    Fixed(usize, usize),
}

fn parse_format(spec: &str) -> Option<Format> {
    let (int, dec) = match spec.split_once('.') {
        Some((i, d)) => (i, Some(d)),
        None => (spec, None),
    };
    if !int.chars().all(|c| c == '#') {
        return None;
    }
    match dec {
        None if !int.is_empty() => Some(Format::Int(int.len())),
        Some(d) if !d.is_empty() && d.chars().all(|c| c == '#') => {
            Some(Format::Fixed(int.len(), d.len()))
        }
        _ => None,
    }
}

/// Python's `repr(float)` (shortest round-trip digits; exponent form below 1e-4 and from
/// 1e16), which Blender's unformatted float variables print like.
pub fn py_float_repr(x: f64) -> String {
    if x.is_nan() {
        return "nan".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "inf".into() } else { "-inf".into() };
    }
    if x == 0.0 {
        return if x.is_sign_negative() { "-0.0" } else { "0.0" }.into();
    }
    // `{:e}` gives the shortest round-trip digits: `2.9970028570928392e1`.
    let e = format!("{x:e}");
    let (mantissa, exp) = e
        .split_once('e')
        .expect("invariant: LowerExp has an exponent");
    let exp: i32 = exp
        .parse()
        .expect("invariant: LowerExp exponent is an integer");
    let (sign, mantissa) = match mantissa.strip_prefix('-') {
        Some(m) => ("-", m),
        None => ("", mantissa),
    };
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    if !(-4..16).contains(&exp) {
        let mut m = digits[..1].to_owned();
        if digits.len() > 1 {
            m.push('.');
            m.push_str(&digits[1..]);
        }
        let es = if exp < 0 { '-' } else { '+' };
        return format!("{sign}{m}e{es}{:02}", exp.abs());
    }
    let point = exp + 1; // digits before the decimal point
    let s = if point <= 0 {
        format!("0.{}{digits}", "0".repeat((-point) as usize))
    } else if point as usize >= digits.len() {
        format!("{digits}{}.0", "0".repeat(point as usize - digits.len()))
    } else {
        format!(
            "{}.{}",
            &digits[..point as usize],
            &digits[point as usize..]
        )
    };
    format!("{sign}{s}")
}

fn format_value(v: &TemplateValue, f: Option<Format>) -> Option<String> {
    Some(match (v, f) {
        (TemplateValue::Str(s), None) => s.clone(),
        (TemplateValue::Str(_), Some(_)) => return None,
        (TemplateValue::Int(i), None) => i.to_string(),
        (TemplateValue::Float(x), None) => py_float_repr(*x),
        (TemplateValue::Int(i), Some(Format::Int(w))) => format!("{i:0w$}"),
        (TemplateValue::Float(x), Some(Format::Int(w))) => format!("{x:0w$.0}"),
        (TemplateValue::Int(i), Some(Format::Fixed(w, p))) => {
            format!("{:0width$.p$}", *i as f64, width = w + 1 + p)
        }
        (TemplateValue::Float(x), Some(Format::Fixed(w, p))) => {
            format!("{x:0width$.p$}", width = w + 1 + p)
        }
    })
}

/// Applies the templates of `path`; `Err` holds every parse error, in order (without the
/// leading `- `).
pub fn apply(path: &str, vars: &TemplateVars) -> Result<String, Vec<String>> {
    let chars: Vec<char> = path.chars().collect();
    let mut out = String::with_capacity(path.len());
    let mut errors = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '{' if chars.get(i + 1) == Some(&'{') => {
                out.push('{');
                i += 2;
            }
            '}' if chars.get(i + 1) == Some(&'}') => {
                out.push('}');
                i += 2;
            }
            '}' => {
                errors.push("Unescaped curly brace '}'.".to_owned());
                i += 1;
            }
            '{' => {
                let mut j = i + 1;
                while j < chars.len() && chars[j] != '}' && chars[j] != '{' {
                    j += 1;
                }
                if j >= chars.len() || chars[j] == '{' {
                    let text: String = chars[i..j].iter().collect();
                    errors.push(format!(
                        "Invalid or incomplete template expression '{text}'."
                    ));
                    i = j;
                    continue;
                }
                let expr: String = chars[i..=j].iter().collect();
                let inner: String = chars[i + 1..j].iter().collect();
                i = j + 1;
                let (name, format) = match inner.split_once(':') {
                    Some((n, spec)) => match parse_format(spec) {
                        Some(f) => (n, Some(f)),
                        None => {
                            errors.push(format!(
                                "Invalid format specifier in template expression '{expr}'."
                            ));
                            continue;
                        }
                    },
                    None => (inner.as_str(), None),
                };
                let Some(v) = vars.get(name) else {
                    errors.push(format!(
                        "Unknown variable referenced in template expression '{expr}'."
                    ));
                    continue;
                };
                match format_value(v, format) {
                    Some(s) => out.push_str(&s),
                    None => errors.push(format!(
                        "Invalid format specifier in template expression '{expr}'."
                    )),
                }
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    if errors.is_empty() {
        Ok(out)
    } else {
        Err(errors)
    }
}

/// The operator's error message for template errors (`RuntimeError: Error: …`, without the
/// `Error: ` prefix the operator framework adds).
pub fn error_message(raw: &str, errors: &[String]) -> String {
    let mut m = format!("Parse errors in path '{raw}':");
    for e in errors {
        m.push_str("\n- ");
        m.push_str(e);
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars() -> TemplateVars {
        TemplateVars::render(None, "Scene", Some("Camera"), [16, 8], 24, 1.0)
    }

    fn ok(p: &str) -> String {
        apply(p, &vars()).unwrap_or_else(|e| panic!("{p}: {e:?}"))
    }

    fn err(p: &str) -> Vec<String> {
        apply(p, &vars()).expect_err(p)
    }

    // Every expectation below was produced by Blender 5.2.4 (`render.render(write_still=True)`
    // with `render.filepath` set to the template, file names listed afterwards).
    #[test]
    fn variables_and_formats() {
        assert_eq!(ok("{scene_name}"), "Scene");
        assert_eq!(ok("{camera_name}"), "Camera");
        assert_eq!(ok("{blend_name}"), "Unsaved");
        assert_eq!(ok("{fps}"), "24.0");
        assert_eq!(ok("{resolution_x}x{resolution_y}"), "16x8");
        assert_eq!(ok("{resolution_x:####}"), "0016");
        assert_eq!(ok("{resolution_x:#}"), "16");
        assert_eq!(ok("{resolution_x:.##}"), "16.00");
        assert_eq!(ok("{resolution_x:###.##}"), "016.00");
        assert_eq!(ok("{resolution_x:##.##}"), "16.00");
        assert_eq!(ok("{fps:#}"), "24");
        assert_eq!(ok("{fps:##}"), "24");
        assert_eq!(ok("{fps:###.#}"), "024.0");
        assert_eq!(
            ok("{resolution_x:#########################}"),
            "0000000000000000000000016"
        );
        assert_eq!(ok("{scene_name}{camera_name}"), "SceneCamera");
        assert_eq!(ok("a/{scene_name}/b"), "a/Scene/b");
    }

    #[test]
    fn fps_values() {
        let v = |fps, base| TemplateVars::render(None, "S", None, [1, 1], fps, base);
        assert_eq!(apply("{fps}", &v(30, 1.001)).unwrap(), "29.970028570928392");
        assert_eq!(apply("{fps}", &v(25, 1.0)).unwrap(), "25.0");
        assert_eq!(apply("{fps}", &v(25, 3.0)).unwrap(), "8.333333333333334");
        assert_eq!(apply("{fps:#}", &v(30, 1.001)).unwrap(), "30");
        assert_eq!(apply("{fps:##.#}", &v(30, 1.001)).unwrap(), "30.0");
        assert_eq!(apply("{fps:.###}", &v(30, 1.001)).unwrap(), "29.970");
    }

    #[test]
    fn escapes() {
        assert_eq!(ok("{{"), "{");
        assert_eq!(ok("}}"), "}");
        assert_eq!(ok("a{{b}}c"), "a{b}c");
        assert_eq!(ok("{{{resolution_x}}}"), "{16}");
        assert_eq!(ok("{resolution_x}{{"), "16{");
    }

    #[test]
    fn errors() {
        let unknown =
            |e: &str| format!("Unknown variable referenced in template expression '{e}'.");
        let invalid = |e: &str| format!("Invalid format specifier in template expression '{e}'.");
        let incomplete = |e: &str| format!("Invalid or incomplete template expression '{e}'.");
        let brace = "Unescaped curly brace '}'.".to_owned();
        for name in [
            "{fps_base}",
            "{fps_float}",
            "{frame}",
            "{frame_current}",
            "{view_layer_name}",
            "{image_name}",
            "{node_name}",
            "{output_path}",
            "{blend_name_short}",
            "{}",
            "{ resolution_x }",
            "{RESOLUTION_X}",
            "{:##}",
            "{fps_float:##.###}",
        ] {
            assert_eq!(err(name), [unknown(name)], "{name}");
        }
        for e in [
            "{fps_float:.2}",
            "{fps:.2}",
            "{resolution_x:}",
            "{resolution_x:abc}",
            "{resolution_x:###.}",
            "{resolution_x:.}",
            "{resolution_x:-#}",
            "{resolution_x:0#}",
            "{resolution_x::#}",
            "{resolution_x:# }",
            "{resolution_x:##.##.#}",
            "{scene_name:##}",
            "{blend_name:#}",
            "{scene_name:}",
            "{nope:x}",
        ] {
            assert_eq!(err(e), [invalid(e)], "{e}");
        }
        assert_eq!(err("{x"), [incomplete("{x")]);
        assert_eq!(err("x{"), [incomplete("{")]);
        assert_eq!(err("{resolution_x"), [incomplete("{resolution_x")]);
        assert_eq!(err("{resolution_x}{"), [incomplete("{")]);
        assert_eq!(err("{a{b}"), [incomplete("{a"), unknown("{b}")]);
        assert_eq!(err("x}"), std::slice::from_ref(&brace));
        assert_eq!(err("}"), std::slice::from_ref(&brace));
        assert_eq!(err("}}}"), std::slice::from_ref(&brace));
        assert_eq!(err("{{a}"), std::slice::from_ref(&brace));
        assert_eq!(err("{a}}"), [unknown("{a}"), brace.clone()]);
        assert_eq!(err("{a}b}"), [unknown("{a}"), brace]);
        assert_eq!(
            err("{nope}{nope2}"),
            [unknown("{nope}"), unknown("{nope2}")]
        );
        assert_eq!(err("{resolution_x:##}{nope:x}"), [invalid("{nope:x}")]);
    }

    #[test]
    fn message() {
        assert_eq!(
            error_message("o48/{nope}{nope2}", &err("{nope}{nope2}")),
            "Parse errors in path 'o48/{nope}{nope2}':\n\
             - Unknown variable referenced in template expression '{nope}'.\n\
             - Unknown variable referenced in template expression '{nope2}'."
        );
    }

    #[test]
    fn safe_names() {
        // `camera.name = 'Cam/Sl'` → `Cam_Sl`, `scene.name = 'Sc:ene'` → `Sc_ene` (5.2.4).
        assert_eq!(make_safe_filename("Cam/Sl"), "Cam_Sl");
        assert_eq!(make_safe_filename("Sc:ene"), "Sc_ene");
        assert_eq!(make_safe_filename("my file"), "my file");
        assert_eq!(
            make_safe_filename("a*b?c\"d<e>f|g\\h\u{1}"),
            "a_b_c_d_e_f_g_h_"
        );
        assert_eq!(make_safe_filename(".."), "__");
        assert_eq!(make_safe_filename(".a"), ".a");
        let v = TemplateVars::render(Some("my file"), "Sc:ene", Some("Cam/Sl"), [1, 1], 24, 1.0);
        assert_eq!(
            apply("{camera_name}-{scene_name}-{blend_name}", &v).unwrap(),
            "Cam_Sl-Sc_ene-my file"
        );
    }

    #[test]
    fn python_float_repr() {
        assert_eq!(py_float_repr(24.0), "24.0");
        assert_eq!(py_float_repr(0.5), "0.5");
        assert_eq!(py_float_repr(1e-6), "1e-06");
        assert_eq!(py_float_repr(0.0001), "0.0001");
        assert_eq!(py_float_repr(1e16), "1e+16");
        assert_eq!(py_float_repr(1.5e16), "1.5e+16");
        assert_eq!(py_float_repr(123456789012345.0), "123456789012345.0");
        assert_eq!(py_float_repr(-2.5), "-2.5");
        assert_eq!(py_float_repr(0.0), "0.0");
    }
}
