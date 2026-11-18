// SPDX-License-Identifier: GPL-3.0-or-later
//! `invariants` (T3, §20.4): properties a case declares with `check=…`, verified on the frap
//! mesh (and on the Blender mesh, which must satisfy them too, or the case is wrong).
//!
//! | Check | Meaning |
//! |---|---|
//! | `manifold` | every edge is used by exactly two faces |
//! | `euler:<n>` | V − E + F = n |
//! | `verts:<a>-<b>`, `edges:<a>-<b>`, `faces:<a>-<b>` | count bands (inclusive; `<a>` alone = exact) |
//! | `uv01` | every UV of every layer lies in [0, 1] |
//! | `uv_no_overlap` | no two UV triangles overlap (fan triangulation, 1e-6 slack) |
//! | `uv_area:<a>-<b>` | total UV area of the first layer within [a, b] |

use std::collections::HashMap;

use crate::GMesh;
use crate::case::Mismatch;

/// Comparator name.
pub const NAME: &str = "invariants";

fn band(spec: &str) -> Option<(f64, f64)> {
    match spec.split_once('-') {
        Some((a, b)) => Some((a.parse().ok()?, b.parse().ok()?)),
        None => {
            let v = spec.parse().ok()?;
            Some((v, v))
        }
    }
}

fn uv_triangles(m: &GMesh) -> Vec<[[f32; 2]; 3]> {
    let Some(uv) = m.uv_layers.values().next() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut c = 0;
    for f in &m.faces {
        if c + f.len() > uv.len() {
            break;
        }
        for k in 1..f.len().saturating_sub(1) {
            out.push([uv[c], uv[c + k], uv[c + k + 1]]);
        }
        c += f.len();
    }
    out
}

fn area2(t: &[[f32; 2]; 3]) -> f64 {
    let [a, b, c] = t.map(|p| [p[0] as f64, p[1] as f64]);
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

/// Strict overlap of two triangles' interiors (separating axis test with slack `eps`).
fn overlap(a: &[[f32; 2]; 3], b: &[[f32; 2]; 3], eps: f64) -> bool {
    for (t, o) in [(a, b), (b, a)] {
        for i in 0..3 {
            let p = t[i].map(|x| x as f64);
            let q = t[(i + 1) % 3].map(|x| x as f64);
            let n = [q[1] - p[1], p[0] - q[0]];
            let proj = |v: [f32; 2]| n[0] * v[0] as f64 + n[1] * v[1] as f64;
            let (tmin, tmax) = t
                .iter()
                .map(|v| proj(*v))
                .fold((f64::MAX, f64::MIN), |(lo, hi), x| (lo.min(x), hi.max(x)));
            let (omin, omax) = o
                .iter()
                .map(|v| proj(*v))
                .fold((f64::MAX, f64::MIN), |(lo, hi), x| (lo.min(x), hi.max(x)));
            if tmax <= omin + eps || omax <= tmin + eps {
                return false;
            }
        }
    }
    true
}

/// Verifies `checks` on `mesh`; `who` is `frap` or `blender`.
pub fn check(who: &str, what: &str, mesh: &GMesh, checks: &[String]) -> Vec<Mismatch> {
    let mut out = Vec::new();
    let mut fail = |c: &str, detail: String| {
        out.push(Mismatch::new(NAME, format!("{what} ({who}) {c}"), detail));
    };
    for c in checks {
        let (name, arg) = match c.split_once(':') {
            Some((n, a)) => (n, Some(a)),
            None => (c.as_str(), None),
        };
        match (name, arg) {
            ("manifold", None) => {
                let mut uses: HashMap<[u32; 2], u32> = HashMap::new();
                for f in &mesh.faces {
                    for i in 0..f.len() {
                        let (a, b) = (f[i], f[(i + 1) % f.len()]);
                        *uses.entry([a.min(b), a.max(b)]).or_default() += 1;
                    }
                }
                let bad = mesh
                    .edges
                    .iter()
                    .filter(|e| uses.get(&[e[0].min(e[1]), e[0].max(e[1])]) != Some(&2))
                    .count();
                if bad > 0 {
                    fail(c, format!("{bad} edges are not used by exactly two faces"));
                }
            }
            ("euler", Some(n)) => {
                let chi =
                    mesh.positions.len() as i64 - mesh.edges.len() as i64 + mesh.faces.len() as i64;
                match n.parse::<i64>() {
                    Ok(n) if n == chi => {}
                    Ok(n) => fail(c, format!("V - E + F = {chi}, expected {n}")),
                    Err(_) => fail(c, "bad argument".into()),
                }
            }
            ("verts" | "edges" | "faces", Some(b)) => {
                let n = match name {
                    "verts" => mesh.positions.len(),
                    "edges" => mesh.edges.len(),
                    _ => mesh.faces.len(),
                } as f64;
                match band(b) {
                    Some((lo, hi)) if (lo..=hi).contains(&n) => {}
                    Some(_) => fail(c, format!("{name} = {n}")),
                    None => fail(c, "bad band".into()),
                }
            }
            ("uv01", None) => {
                for (layer, uv) in &mesh.uv_layers {
                    let bad = uv
                        .iter()
                        .filter(|p| p.iter().any(|x| !(0.0..=1.0).contains(x)))
                        .count();
                    if bad > 0 {
                        fail(c, format!("{bad} UVs of {layer:?} outside [0, 1]"));
                    }
                }
            }
            ("uv_no_overlap", None) => {
                let tris: Vec<_> = uv_triangles(mesh)
                    .into_iter()
                    .filter(|t| area2(t).abs() > 1e-12)
                    .collect();
                let mut pairs = 0;
                for i in 0..tris.len() {
                    for j in i + 1..tris.len() {
                        if overlap(&tris[i], &tris[j], 1e-6) {
                            pairs += 1;
                        }
                    }
                }
                if pairs > 0 {
                    fail(c, format!("{pairs} overlapping UV triangle pairs"));
                }
            }
            ("uv_area", Some(b)) => {
                let a: f64 = uv_triangles(mesh)
                    .iter()
                    .map(|t| area2(t).abs() / 2.0)
                    .sum();
                match band(b) {
                    Some((lo, hi)) if a >= lo && a <= hi => {}
                    Some(_) => fail(c, format!("UV area = {a}")),
                    None => fail(c, "bad band".into()),
                }
            }
            _ => fail(c, "unknown check".into()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compare::mesh_exact::tests::cube;

    fn checks(s: &[&str]) -> Vec<String> {
        s.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn cube_invariants_hold() {
        let mut m = cube();
        // A non-overlapping cube UV layout: six cells of a 3x2 grid.
        let mut uv = Vec::new();
        for f in 0..6 {
            let (x, y) = ((f % 3) as f32 / 3.0, (f / 3) as f32 / 2.0);
            uv.extend([
                [x, y],
                [x + 1.0 / 3.0, y],
                [x + 1.0 / 3.0, y + 0.5],
                [x, y + 0.5],
            ]);
        }
        m.uv_layers.insert("UVMap".into(), uv);
        let c = checks(&[
            "manifold",
            "euler:2",
            "verts:8",
            "faces:1-6",
            "edges:12-12",
            "uv01",
            "uv_no_overlap",
            "uv_area:0.99-1.01",
        ]);
        assert_eq!(check("frap", "m", &m, &c), vec![]);
    }

    #[test]
    fn one_change_breaks_an_invariant() {
        let m = cube();
        let mut open = m.clone();
        open.faces.pop();
        assert_eq!(check("frap", "m", &open, &checks(&["manifold"])).len(), 1);
        assert_eq!(check("frap", "m", &open, &checks(&["euler:2"])).len(), 1);
        let mut uv = m.clone();
        uv.uv_layers.get_mut("UVMap").unwrap()[0] = [1.5, 0.0];
        assert_eq!(check("frap", "m", &uv, &checks(&["uv01"])).len(), 1);
        let mut ov = m.clone();
        let quad = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        ov.uv_layers
            .insert("UVMap".into(), (0..24).map(|i| quad[i % 4]).collect());
        assert_eq!(
            check("frap", "m", &ov, &checks(&["uv_no_overlap"])).len(),
            1
        );
        assert_eq!(check("frap", "m", &m, &checks(&["nope"])).len(), 1);
    }
}
