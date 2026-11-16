// SPDX-License-Identifier: GPL-3.0-or-later
//! Studio light rigs for Workbench `light='STUDIO'`, baked from Blender 5.2.4 into
//! `assets/studio_lights/*.sl` (Blender's own text format; provenance in
//! `assets/studio_lights/README.md`). Parsing follows `BKE_studiolight.cc`
//! (`studiolight_load_solid_light`): `key value` lines, unknown keys ignored.

/// One of the four solid lights of a studio rig (`SolidLight`).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct SolidLight {
    /// `flag` (`UserSolidLight.use`).
    pub use_light: bool,
    /// `smooth` (wrap).
    pub smooth: f32,
    /// `col` (`diffuse_color`).
    pub diffuse: [f32; 3],
    /// `spec` (`specular_color`).
    pub specular: [f32; 3],
    /// `vec` (`direction`, view space unless world-space lighting is on).
    pub direction: [f32; 3],
}

/// A STUDIO studio light (`StudioLight` with `STUDIOLIGHT_TYPE_STUDIO`).
#[derive(Clone, Debug, PartialEq)]
pub struct StudioLight {
    /// Name as Blender lists it (`Default`, `basic.sl`, …).
    pub name: &'static str,
    /// `light_ambient`
    pub ambient: [f32; 3],
    /// `light[0..4]`
    pub lights: [SolidLight; 4],
    /// `STUDIOLIGHT_SPECULAR_HIGHLIGHT_PASS` (true for every shipped rig, verified).
    pub specular_highlight_pass: bool,
}

/// The shipped rigs, in Blender's `preferences.studio_lights` order.
const RIGS: &[(&str, &str)] = &[
    (
        "Default",
        include_str!("../../../../assets/studio_lights/Default.sl"),
    ),
    (
        "basic.sl",
        include_str!("../../../../assets/studio_lights/basic.sl"),
    ),
    (
        "outdoor.sl",
        include_str!("../../../../assets/studio_lights/outdoor.sl"),
    ),
    (
        "paint.sl",
        include_str!("../../../../assets/studio_lights/paint.sl"),
    ),
    (
        "rim.sl",
        include_str!("../../../../assets/studio_lights/rim.sl"),
    ),
    (
        "studio.sl",
        include_str!("../../../../assets/studio_lights/studio.sl"),
    ),
];

/// Names of the shipped STUDIO rigs (`View3DShading.studio_light` enum items).
pub fn names() -> impl Iterator<Item = &'static str> {
    RIGS.iter().map(|(n, _)| *n)
}

/// The rig called `name` (`None` when unknown).
pub fn get(name: &str) -> Option<StudioLight> {
    RIGS.iter()
        .find(|(n, _)| *n == name)
        .map(|(n, text)| parse(n, text))
}

/// The factory rig (`scene.display.shading.studio_light == 'Default'`, verified).
pub fn default_rig() -> StudioLight {
    get("Default").expect("invariant: Default.sl is embedded")
}

fn parse(name: &'static str, text: &str) -> StudioLight {
    let mut sl = StudioLight {
        name,
        ambient: [0.0; 3],
        lights: [SolidLight::default(); 4],
        specular_highlight_pass: true,
    };
    let axis = |c: &str| match c {
        "x" => Some(0),
        "y" => Some(1),
        "z" => Some(2),
        _ => None,
    };
    for line in text.lines() {
        let Some((key, val)) = line.split_once(' ') else {
            continue;
        };
        let Ok(v) = val.trim().parse::<f32>() else {
            continue;
        };
        if let Some(c) = key.strip_prefix("light_ambient.") {
            if let Some(i) = axis(c) {
                sl.ambient[i] = v;
            }
            continue;
        }
        let Some(rest) = key.strip_prefix("light[") else {
            continue;
        };
        let Some((idx, field)) = rest.split_once("].") else {
            continue;
        };
        let Some(l) = idx.parse::<usize>().ok().and_then(|i| sl.lights.get_mut(i)) else {
            continue;
        };
        match field.split_once('.') {
            None if field == "flag" => l.use_light = v != 0.0,
            None if field == "smooth" => l.smooth = v,
            Some((vec, c)) => {
                let Some(i) = axis(c) else { continue };
                match vec {
                    "col" => l.diffuse[i] = v,
                    "spec" => l.specular[i] = v,
                    "vec" => l.direction[i] = v,
                    _ => {}
                }
            }
            None => {}
        }
    }
    sl
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_rig_matches_blender_dump() {
        // bpy.context.preferences.studio_lights['Default'] on 5.2.4 (factory startup).
        let d = default_rig();
        assert_eq!(d.ambient, [0.0; 3]);
        assert!(d.lights.iter().all(|l| l.use_light));
        assert_eq!(d.lights[0].smooth, 0.526620);
        assert_eq!(d.lights[1].diffuse, [0.521083, 0.538226, 0.538226]);
        assert_eq!(d.lights[1].specular, [0.599030; 3]);
        assert_eq!(d.lights[1].direction, [-0.408163, 0.346939, 0.844415]);
        assert_eq!(d.lights[3].smooth, 0.2);
        assert!(d.specular_highlight_pass);
    }

    #[test]
    fn every_rig_parses() {
        let names: Vec<_> = names().collect();
        assert_eq!(
            names,
            [
                "Default",
                "basic.sl",
                "outdoor.sl",
                "paint.sl",
                "rim.sl",
                "studio.sl"
            ]
        );
        for n in names {
            let r = get(n).unwrap();
            assert!(r.lights.iter().any(|l| l.use_light), "{n}");
        }
        let o = get("outdoor.sl").unwrap();
        assert_eq!(o.ambient, [0.016682, 0.022491, 0.025394]);
        assert!(!o.lights[3].use_light);
        assert!(get("nope.sl").is_none());
    }
}
