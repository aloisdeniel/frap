// SPDX-License-Identifier: GPL-3.0-or-later
//! The sequence generator: from a seed, statements of the bpy-line subset both tools run
//! (WP 8.7).
//!
//! The generator follows a rough model of the scene (the mode, object names in creation
//! order, modifier and vertex group names) so most statements apply; the model is only a
//! guide: a statement that fails (a poll, a missing name) must fail the same way in both
//! tools, which the `lines` comparator checks. Every statement is one line, so minimisation
//! works line by line.

use std::collections::BTreeMap;

use super::catalog::{Catalog, Kind, OpEntry, PropGen, RnaEntry};
use super::rng::Rng;

/// The generator's model of the scene.
#[derive(Clone, Debug)]
pub struct Model {
    /// Mesh Edit Mode (else Object Mode).
    pub edit: bool,
    /// Object names, creation order (the factory scene first).
    pub objects: Vec<String>,
    /// Modifier names added so far (any object).
    pub modifiers: Vec<String>,
    /// Vertex group names added so far (any object).
    pub vgroups: Vec<String>,
    /// The active object is (probably) a mesh: Edit Mode is only entered then.
    pub active_mesh: bool,
    /// Next `.NNN` suffix per base name.
    counts: BTreeMap<String, u32>,
}

impl Default for Model {
    fn default() -> Model {
        let mut counts = BTreeMap::new();
        for n in ["Cube", "Light", "Camera"] {
            counts.insert(n.to_owned(), 1);
        }
        Model {
            edit: false,
            objects: vec!["Cube".into(), "Light".into(), "Camera".into()],
            modifiers: Vec::new(),
            vgroups: Vec::new(),
            active_mesh: true,
            counts,
        }
    }
}

impl Model {
    /// Blender's next unique name for `base` (`Cube`, `Cube.001`, …) in this model.
    fn unique(&mut self, base: &str) -> String {
        let n = self.counts.entry(base.to_owned()).or_insert(0);
        let name = if *n == 0 {
            base.to_owned()
        } else {
            format!("{base}.{:03}", *n)
        };
        *n += 1;
        name
    }

    fn add_object(&mut self, base: &str) {
        let name = self.unique(base);
        self.active_mesh = is_mesh_name(&name);
        self.objects.push(name);
    }
}

/// The object base name a primitive or add operator creates (Object Mode only).
fn created_object(idname: &str, args: &[(String, String)]) -> Option<&'static str> {
    let arg = |k: &str| args.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
    Some(match idname {
        "mesh.primitive_plane_add" => "Plane",
        "mesh.primitive_cube_add" => "Cube",
        "mesh.primitive_circle_add" => "Circle",
        "mesh.primitive_uv_sphere_add" => "Sphere",
        "mesh.primitive_ico_sphere_add" => "Icosphere",
        "mesh.primitive_cylinder_add" => "Cylinder",
        "mesh.primitive_cone_add" => "Cone",
        "mesh.primitive_torus_add" => "Torus",
        "mesh.primitive_grid_add" => "Grid",
        "mesh.primitive_monkey_add" => "Suzanne",
        "object.camera_add" => "Camera",
        "object.light_add" => match arg("type") {
            Some("'SUN'") => "Sun",
            Some("'SPOT'") => "Spot",
            Some("'AREA'") => "Area",
            _ => "Point",
        },
        "object.empty_add" => "Empty",
        "object.add" => match arg("type") {
            Some("'EMPTY'") => "Empty",
            _ => "Mesh",
        },
        _ => return None,
    })
}

/// Whether an object of this (generated) name is a mesh.
fn is_mesh_name(name: &str) -> bool {
    let base = name.split('.').next().unwrap_or(name);
    !matches!(
        base,
        "Light" | "Camera" | "Empty" | "Point" | "Sun" | "Spot" | "Area"
    )
}

/// The default name `object.modifier_add(type=…)` gives.
pub fn modifier_default_name(ty: &str) -> &'static str {
    match ty {
        "SUBSURF" => "Subdivision",
        "MIRROR" => "Mirror",
        "ARRAY" => "Array",
        "BEVEL" => "Bevel",
        "SOLIDIFY" => "Solidify",
        "TRIANGULATE" => "Triangulate",
        "DECIMATE" => "Decimate",
        "WELD" => "Weld",
        "DISPLACE" => "Displace",
        "EDGE_SPLIT" => "EdgeSplit",
        "WEIGHTED_NORMAL" => "WeightedNormal",
        "SMOOTH" => "Smooth",
        "CAST" => "Cast",
        "SIMPLE_DEFORM" => "SimpleDeform",
        "WIREFRAME" => "Wireframe",
        "SCREW" => "Screw",
        "BUILD" => "Build",
        "LAPLACIANSMOOTH" => "LaplacianSmooth",
        _ => "Modifier",
    }
}

/// A float with at most two decimals (`0.35`, `-1.0`).
fn float(rng: &mut Rng, lo: f64, hi: f64) -> String {
    let x = lo + (hi - lo) * rng.unit();
    let r = (x * 100.0).round() / 100.0;
    let r = if r == 0.0 { 0.0 } else { r };
    let s = format!("{r}");
    if s.contains('.') { s } else { format!("{s}.0") }
}

fn tuple(items: Vec<String>) -> String {
    format!("({})", items.join(", "))
}

fn py_bool(b: bool) -> String {
    if b { "True" } else { "False" }.to_owned()
}

fn py_str(s: &str) -> String {
    format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'"))
}

/// One value of `kind`.
fn value(rng: &mut Rng, model: &Model, kind: &Kind) -> String {
    let many = |n: usize, rng: &mut Rng, f: &mut dyn FnMut(&mut Rng) -> String| {
        if n == 0 {
            f(rng)
        } else {
            tuple((0..n).map(|_| f(rng)).collect())
        }
    };
    match kind {
        Kind::Bool { n } => many(*n, rng, &mut |r| py_bool(r.chance(0.5))),
        Kind::Int { lo, hi, n } => many(*n, rng, &mut |r| r.range_i64(*lo, *hi).to_string()),
        Kind::Float { lo, hi, n } => many(*n, rng, &mut |r| float(r, *lo, *hi)),
        Kind::Enum(items) => py_str(rng.pick(items)),
        Kind::EnumFlag(items) => {
            let mut chosen: Vec<&String> = items.iter().filter(|_| rng.chance(0.4)).collect();
            if chosen.is_empty() {
                chosen.push(rng.pick(items));
            }
            let inner: Vec<String> = chosen.iter().map(|s| py_str(s)).collect();
            format!("{{{}}}", inner.join(", "))
        }
        Kind::Literal(items) => {
            let t = rng.pick(items).clone();
            expand(rng, model, &t)
        }
    }
}

/// The arguments of one call of `op`: each property with probability ½ (always when `!`).
fn args(rng: &mut Rng, model: &Model, props: &[PropGen]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for p in props {
        if p.always || rng.chance(0.5) {
            out.push((p.name.clone(), value(rng, model, &p.kind)));
        }
    }
    out
}

/// Splits the content of `$c(…)` at top-level `|`.
fn split_choices(s: &str) -> Vec<&str> {
    let (mut depth, mut start, mut out) = (0i32, 0, Vec::new());
    for (i, c) in s.char_indices() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            '|' if depth == 0 => {
                out.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&s[start..]);
    out
}

/// The text up to the `)` closing an argument list that starts at `s[0]` (after `(`).
fn until_close(s: &str) -> Option<(&str, &str)> {
    let mut depth = 0i32;
    for (i, c) in s.char_indices() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' if depth == 0 => return Some((&s[..i], &s[i + 1..])),
            ')' | ']' | '}' => depth -= 1,
            _ => {}
        }
    }
    None
}

fn bounds(args: &str) -> (f64, f64) {
    let (a, b) = args.split_once(',').unwrap_or((args, args));
    (
        a.trim().parse().unwrap_or(0.0),
        b.trim().parse().unwrap_or(1.0),
    )
}

/// Expands the template placeholders of `t` (see the catalogue module docs).
pub fn expand(rng: &mut Rng, model: &Model, t: &str) -> String {
    let mut out = String::new();
    let mut rest = t;
    while let Some(i) = rest.find('$') {
        out.push_str(&rest[..i]);
        rest = &rest[i + 1..];
        let word: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect();
        rest = &rest[word.len()..];
        let call = rest
            .strip_prefix('(')
            .and_then(until_close)
            .filter(|_| matches!(word.as_str(), "f" | "i" | "v3" | "c"));
        let inner = match call {
            Some((inner, after)) => {
                rest = after;
                inner
            }
            None => "",
        };
        let v = match word.as_str() {
            "f" => {
                let (lo, hi) = bounds(inner);
                float(rng, lo, hi)
            }
            "i" => {
                let (lo, hi) = bounds(inner);
                rng.range_i64(lo as i64, hi as i64).to_string()
            }
            "v3" => {
                let (lo, hi) = bounds(inner);
                tuple((0..3).map(|_| float(rng, lo, hi)).collect())
            }
            "b" => py_bool(rng.chance(0.5)),
            "b3" => tuple((0..3).map(|_| py_bool(rng.chance(0.5))).collect()),
            "c" => {
                let choices = split_choices(inner);
                let c = choices[rng.below(choices.len())].trim().to_owned();
                expand(rng, model, &c)
            }
            "obj" => py_str(rng.pick(&model.objects)),
            "mod" => py_str(if model.modifiers.is_empty() {
                "Subdivision"
            } else {
                rng.pick(&model.modifiers)
            }),
            "vg" => py_str(if model.vgroups.is_empty() {
                "Group"
            } else {
                rng.pick(&model.vgroups)
            }),
            other => format!("${other}"),
        };
        out.push_str(&v);
    }
    out.push_str(rest);
    out
}

/// The statement calling `op` with `args`.
pub fn call(idname: &str, args: &[(String, String)]) -> String {
    let a: Vec<String> = args.iter().map(|(k, v)| format!("{k}={v}")).collect();
    format!("bpy.ops.{idname}({})", a.join(", "))
}

/// Updates the model after an operator call.
fn observe_op(model: &mut Model, op: &OpEntry, args: &[(String, String)]) {
    let arg = |k: &str| {
        args.iter()
            .find(|(n, _)| n == k)
            .map(|(_, v)| v.trim_matches('\''))
    };
    match op.idname.as_str() {
        "object.modifier_add" => {
            if let Some(t) = arg("type") {
                let name = modifier_default_name(t).to_owned();
                model.modifiers.push(name);
            }
        }
        "object.vertex_group_add" | "object.vertex_group_assign_new" => {
            let n = if model.vgroups.iter().any(|g| g == "Group") {
                format!("Group.{:03}", model.vgroups.len())
            } else {
                "Group".to_owned()
            };
            model.vgroups.push(n);
        }
        "object.duplicate" | "object.duplicate_move" | "object.duplicate_move_linked" => {
            // The duplicates of the selection: at least the active object's copy.
            if let Some(last) = model.objects.first().cloned() {
                let base = last.split('.').next().unwrap_or(&last).to_owned();
                model.add_object(&base);
            }
        }
        id if !model.edit => {
            if let Some(base) = created_object(id, args) {
                model.add_object(base);
            }
        }
        _ => {}
    }
}

fn observe_rna(model: &mut Model, text: &str) {
    if let Some(rest) =
        text.strip_prefix("bpy.context.view_layer.objects.active = bpy.data.objects['")
    {
        model.active_mesh = is_mesh_name(rest.trim_end_matches("']"));
    }
    if let Some(rest) = text.split("vertex_groups.new(name=").nth(1) {
        let name = rest.trim_end_matches(')').trim_matches('\'');
        model.vgroups.push(name.to_owned());
    }
    if let Some(rest) = text.split("modifiers.new(").nth(1)
        && let Some((name, _)) = rest.split_once(',')
    {
        model
            .modifiers
            .push(name.trim().trim_matches('\'').to_owned());
    }
}

/// Generates `len` statements from `seed`.
pub fn generate(seed: u64, len: usize) -> Vec<String> {
    let cat = Catalog::get();
    let mut rng = Rng::new(seed);
    let mut model = Model::default();
    let mut out = Vec::with_capacity(len);
    while out.len() < len {
        let roll = rng.unit();
        if roll < 0.12 && (model.edit || model.active_mesh) {
            let (mode, edit) = if model.edit {
                ("OBJECT", false)
            } else {
                ("EDIT", true)
            };
            out.push(format!("bpy.ops.object.mode_set(mode='{mode}')"));
            model.edit = edit;
            continue;
        }
        if (0.12..0.30).contains(&roll) {
            let pool: Vec<&RnaEntry> = cat
                .rna
                .iter()
                .filter(|r| r.ctx.applies(model.edit))
                .collect();
            if let Some(r) = pool.get(rng.below(pool.len().max(1))) {
                let text = expand(&mut rng, &model, &r.template);
                observe_rna(&mut model, &text);
                out.push(text);
            }
            continue;
        }
        let pool: Vec<&OpEntry> = cat
            .ops
            .iter()
            .filter(|o| o.ctx.applies(model.edit))
            .collect();
        let op = pool[rng.below(pool.len())];
        let a = args(&mut rng, &model, &op.props);
        observe_op(&mut model, op, &a);
        out.push(call(&op.idname, &a));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_expand() {
        let m = Model::default();
        let mut r = Rng::new(3);
        let s = expand(&mut r, &m, "x = $v3(-1,1)");
        assert!(
            s.starts_with("x = (") && s.matches(", ").count() == 2,
            "{s}"
        );
        let s = expand(
            &mut r,
            &m,
            "m = $c((True, False, False)|(False, True, False))",
        );
        assert!(
            s == "m = (True, False, False)" || s == "m = (False, True, False)",
            "{s}"
        );
        let s = expand(&mut r, &m, "bpy.data.objects[$obj].select_set($b)");
        assert!(s.contains("bpy.data.objects['"), "{s}");
        assert_eq!(expand(&mut r, &m, "n = $mod"), "n = 'Subdivision'");
        let i: i64 = expand(&mut r, &m, "$i(2,2)").parse().unwrap();
        assert_eq!(i, 2);
    }

    #[test]
    fn floats_have_two_decimals_at_most() {
        let mut r = Rng::new(9);
        for _ in 0..1000 {
            let s = float(&mut r, -2.0, 2.0);
            let v: f64 = s.parse().unwrap();
            assert!((-2.0..=2.0).contains(&v), "{s}");
            assert!(s.split('.').nth(1).unwrap().len() <= 2, "{s}");
            assert_ne!(s, "-0.0");
        }
    }

    #[test]
    fn names_follow_blender() {
        let mut m = Model::default();
        assert_eq!(m.unique("Cube"), "Cube.001");
        assert_eq!(m.unique("Cube"), "Cube.002");
        assert_eq!(m.unique("Plane"), "Plane");
        assert_eq!(m.unique("Plane"), "Plane.001");
    }

    #[test]
    fn generation_is_deterministic_and_tracks_the_mode() {
        let a = generate(42, 40);
        assert_eq!(a, generate(42, 40));
        assert_ne!(a, generate(43, 40));
        assert_eq!(a.len(), 40);
        let mut edit = false;
        for s in &a {
            if s == "bpy.ops.object.mode_set(mode='EDIT')" {
                edit = true;
            } else if s == "bpy.ops.object.mode_set(mode='OBJECT')" {
                edit = false;
            } else if edit {
                assert!(
                    !s.contains(".data.vertices["),
                    "mesh data write in Edit Mode: {s}"
                );
            }
            assert!(!s.contains('\n'));
        }
    }
}
