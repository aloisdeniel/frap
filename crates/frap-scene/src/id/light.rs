// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.types.Light` (§5.6: generated plain fields per light type; assigning `type` re-types
//! the RNA struct to `PointLight`/`SunLight`/`SpotLight`/`AreaLight`). The field set is frozen
//! at M1 W0 (decision log W0-21); WP 1.10 owns the bindings.
//!
//! `LightFields` is the union of every light type's fields, as in Blender's DNA, so changing
//! `light.type` keeps every value (verified: `energy` survives a POINT → SPOT → POINT round
//! trip). The binding is hand-written because the struct is refined by `type`: a property is
//! visible only when the refined struct (or `Light`/`ID`) defines it, exactly as in Blender
//! (`AttributeError: 'PointLight' object has no attribute 'spot_size'`).
//!
//! M3 (WP 3.4): the read-only `temperature_color` (the blackbody colour of `temperature` with
//! `use_temperature`, white otherwise) and [`Light::area`] (`Light.area(matrix_world)`, used by
//! the glTF exporter for lights that are not `normalize`d).

use frap_anim::AnimData;
use frap_rna::*;
use glam::Mat4;
use smol_str::SmolStr;

use crate::generated::plain::LightFields;

/// Light data.
#[derive(Clone, Debug)]
pub struct Light {
    /// ID header.
    pub header: IdHeader,
    /// `Light.type` ("POINT", "SUN", "SPOT", "AREA")
    pub r#type: SmolStr,
    /// The plain RNA fields of every light type (union, as in Blender's DNA; generated,
    /// WP 1.1).
    pub fields: LightFields,
    /// `Light.animation_data`
    pub animation_data: Option<AnimData>,
}

impl Light {
    /// Light data with the fresh values of `bpy.data.lights.new(name, type)` (10 W,
    /// `shadow_soft_size` 0, verified; the factory Light is 1000 W).
    pub fn new(name: &str, r#type: &str) -> Light {
        Light {
            header: IdHeader::new(name),
            r#type: r#type.into(),
            fields: LightFields::default(),
            animation_data: None,
        }
    }
    /// The RNA struct of a light type (`PointLight`, …; `Light` for an unknown type).
    pub fn srna_of(ty: &str) -> &'static StructDef {
        match ty {
            "POINT" => &frap_spec_rna::POINT_LIGHT,
            "SUN" => &frap_spec_rna::SUN_LIGHT,
            "SPOT" => &frap_spec_rna::SPOT_LIGHT,
            "AREA" => &frap_spec_rna::AREA_LIGHT,
            _ => &frap_spec_rna::LIGHT,
        }
    }
    /// The values the factory `startup.blend` light (`Light`, a POINT light) stores for the
    /// other light types (DNA keeps every type's fields; the startup dump only lists the
    /// PointLight properties). Verified on 5.2.4 by re-typing the factory light: the spot cone
    /// is 75°, the sun angle 11.42°, the area a 0.1 × 0.1 rectangle, the cascade distance 1000.
    pub fn set_factory_startup_hidden_fields(&mut self) {
        let f = &mut self.fields;
        f.spot_size = 1.308_996_9;
        f.angle = 0.199_337_3;
        f.shadow_cascade_max_distance = 1000.0;
        f.shape = SmolStr::new_static("RECTANGLE");
        f.size = 0.1;
        f.size_y = 0.1;
    }
    /// `Light.temperature_color` (`rna_Light_temperature_color_get`): the blackbody colour of
    /// `temperature` with `use_temperature`, else white.
    pub fn temperature_color(&self) -> [f32; 3] {
        if self.fields.use_temperature {
            blackbody_temperature_to_rgb(self.fields.temperature)
        } else {
            [1.0; 3]
        }
    }
    /// `Light.area(matrix_world)` (`BKE_light_area`): the emitting area the energy is spread
    /// over when `normalize` is off. Point and spot lights ignore the object scale (legacy).
    pub fn area(&self, object_to_world: &Mat4) -> f32 {
        let f = &self.fields;
        match self.r#type.as_str() {
            "AREA" => {
                let scale_x = object_to_world.x_axis.truncate().length();
                let scale_y = object_to_world.y_axis.truncate().length();
                let rect_or_ellipse = matches!(f.shape.as_str(), "RECTANGLE" | "ELLIPSE");
                let size_x = f.size * scale_x;
                let size_y = if rect_or_ellipse { f.size_y } else { f.size } * scale_y;
                let mut area = size_x * size_y;
                if matches!(f.shape.as_str(), "DISK" | "ELLIPSE") {
                    area *= std::f32::consts::FRAC_PI_4;
                }
                area
            }
            "SUN" => {
                let angle = f.angle / 2.0;
                if angle > 0.0 {
                    std::f32::consts::PI * (angle.sin() * angle.sin())
                } else {
                    1.0
                }
            }
            // POINT, SPOT
            _ => {
                let radius = f.shadow_soft_size;
                if radius > 0.0 {
                    (4.0 * std::f64::consts::PI) as f32 * (radius * radius)
                } else {
                    4.0
                }
            }
        }
    }
    fn defines(&self, prop: &PropDef) -> bool {
        Light::srna_of(&self.r#type)
            .find_prop(prop.identifier)
            .is_some()
    }
}

impl Rna for Light {
    fn srna(&self) -> &'static StructDef {
        Light::srna_of(&self.r#type)
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        if let Some(r) = super::id_common_get(&self.header, "LIGHT", prop) {
            return r;
        }
        match prop.identifier {
            "type" => return Ok(Field::Value(Value::Enum(self.r#type.clone()))),
            // Light node trees (Cycles light shaders) are not modelled.
            "node_tree" => return Ok(Field::Id(None)),
            "temperature_color" => {
                let c = self.temperature_color();
                return Ok(Field::Value(Value::FloatArray {
                    data: smallvec::SmallVec::from_slice(&c),
                    shape: prop.array,
                    tag: MathTag::for_subtype(prop.subtype),
                }));
            }
            _ => {}
        }
        if self.defines(prop)
            && let Some(f) = self.fields.rna_get(prop)
        {
            return Ok(f);
        }
        Err(super::no_attr(self.srna(), prop))
    }
}

impl RnaMut for Light {
    fn set(&mut self, prop: &'static PropDef, v: Value) -> Result<SetOutcome, RnaError> {
        let srna = self.srna();
        if let Some(r) = super::id_common_set(&mut self.header, srna, prop, &v) {
            return r;
        }
        match prop.identifier {
            "type" => {
                self.r#type = v.as_str().map(SmolStr::new).unwrap_or(self.r#type.clone());
                return Ok(SetOutcome::Done);
            }
            "node_tree" | "temperature_color" => {
                return Err(frap_rna::traits::read_only(srna, prop.identifier));
            }
            _ => {}
        }
        if self.defines(prop)
            && let Some(r) = self.fields.rna_set(prop, v)
        {
            return r;
        }
        Err(super::no_attr(srna, prop))
    }
    fn struct_mut(&mut self, prop: &'static PropDef) -> Result<RnaMutGuard<'_>, RnaError> {
        let srna = self.srna();
        self.fields
            .rna_struct_mut(prop)
            .ok_or_else(|| frap_rna::traits::wrong_access(srna, prop.identifier, "a struct"))
    }
    fn coll_mut(&mut self, prop: &'static PropDef) -> Result<RnaCollMutGuard<'_>, RnaError> {
        Err(frap_rna::traits::wrong_access(
            self.srna(),
            prop.identifier,
            "a collection",
        ))
    }
}

// Ported from blender/source/blender/imbuf/intern/colormanagement.cc (v5.2.4,
// GPL-2.0-or-later): `blackbody_table_*`, `blackbody_temperature_to_rec709`,
// `IMB_colormanagement_blackbody_temperature_to_rgb`. The scene-linear space of the factory
// colour management is Linear Rec.709, so the Rec.709 → scene-linear step is the identity.
#[allow(clippy::excessive_precision)]
const BLACKBODY_R: [[f32; 3]; 7] = [
    [1.61919106e+03, -2.05010916e-03, 5.02995757e+00],
    [2.48845471e+03, -1.11330907e-03, 3.22621544e+00],
    [3.34143193e+03, -4.86551192e-04, 1.76486769e+00],
    [4.09461742e+03, -1.27446582e-04, 7.25731635e-01],
    [4.67028036e+03, 2.91258199e-05, 1.26703442e-01],
    [4.59509185e+03, 2.87495649e-05, 1.50345020e-01],
    [3.78717450e+03, 9.35907826e-06, 3.99075871e-01],
];
#[allow(clippy::excessive_precision)]
const BLACKBODY_G: [[f32; 3]; 7] = [
    [-4.88999748e+02, 6.04330754e-04, -7.55807526e-02],
    [-7.55994277e+02, 3.16730098e-04, 4.78306139e-01],
    [-1.02363977e+03, 1.20223470e-04, 9.36662319e-01],
    [-1.26571316e+03, 4.87340896e-06, 1.27054498e+00],
    [-1.42529332e+03, -4.01150431e-05, 1.43972784e+00],
    [-1.17554822e+03, -2.16378048e-05, 1.30408023e+00],
    [-5.00799571e+02, -4.59832026e-06, 1.09098763e+00],
];
#[allow(clippy::excessive_precision)]
const BLACKBODY_B: [[f32; 4]; 7] = [
    [
        5.96945309e-11,
        -4.85742887e-08,
        -9.70622247e-05,
        -4.07936148e-03,
    ],
    [
        2.40430366e-11,
        5.55021075e-08,
        -1.98503712e-04,
        2.89312858e-02,
    ],
    [
        -1.40949732e-11,
        1.89878968e-07,
        -3.56632824e-04,
        9.10767778e-02,
    ],
    [
        -3.61460868e-11,
        2.84822009e-07,
        -4.93211319e-04,
        1.56723440e-01,
    ],
    [
        -1.97075738e-11,
        1.75359352e-07,
        -2.50542825e-04,
        -2.22783266e-02,
    ],
    [
        -1.61997957e-13,
        -1.64216008e-08,
        3.86216271e-04,
        -7.38077418e-01,
    ],
    [
        6.72650283e-13,
        -2.73078809e-08,
        4.24098264e-04,
        -7.52335691e-01,
    ],
];

/// `blackbody_temperature_to_rec709`, clamped to `[0, FLT_MAX]`.
#[allow(clippy::excessive_precision)]
pub fn blackbody_temperature_to_rgb(t: f32) -> [f32; 3] {
    let rec709 = if t >= 12000.0 {
        [0.8262954810464208, 0.9945080501520986, 1.566307710274283]
    } else if t < 800.0 {
        [5.413294490189271, -0.20319390035873933, -0.0822535242887164]
    } else {
        let i = if t >= 6365.0 {
            6
        } else if t >= 3315.0 {
            5
        } else if t >= 1902.0 {
            4
        } else if t >= 1449.0 {
            3
        } else if t >= 1167.0 {
            2
        } else if t >= 965.0 {
            1
        } else {
            0
        };
        let (r, g, b) = (&BLACKBODY_R[i], &BLACKBODY_G[i], &BLACKBODY_B[i]);
        let t_inv = 1.0 / t;
        [
            r[0] * t_inv + r[1] * t + r[2],
            g[0] * t_inv + g[1] * t + g[2],
            ((b[0] * t + b[1]) * t + b[2]) * t + b[3],
        ]
    };
    rec709.map(|c| c.clamp(0.0, f32::MAX))
}

impl IdType for Light {
    type Kind = LightId;
    fn header(&self) -> &IdHeader {
        &self.header
    }
    fn header_mut(&mut self) -> &mut IdHeader {
        &mut self.header
    }
    fn for_each_ref(&self, f: &mut dyn FnMut(IdAny)) {
        if let Some(a) = self.animation_data.as_ref().and_then(|a| a.action) {
            f(a.any());
        }
    }
    fn remap_ref(&mut self, from: IdAny, to: Option<IdAny>) {
        super::remap_action(&mut self.animation_data, from, to);
    }
}

#[cfg(test)]
// Values are Blender dumps, kept as printed.
#[allow(clippy::excessive_precision, clippy::approx_constant)]
mod tests {
    use glam::{EulerRot, Quat, Vec3};

    use super::*;

    // Dumped from Blender 5.2.4 (`Light.temperature_color`; RNA clamps `temperature` to
    // [800, 40000]).
    #[test]
    fn blackbody_matches_blender() {
        let cases: [(f32, [f32; 3]); 11] = [
            (800.0, [5.413_859_4, 0.0, 0.0]),
            (900.0, [4.983_960_6, 0.0, 0.0]),
            (1000.0, [4.601_361_3, 0.039_041_907, 0.0]),
            (1200.0, [3.965_532_8, 0.227_897_29, 0.0]),
            (1500.0, [3.264_306_8, 0.434_046_33, 0.0]),
            (2000.0, [2.520_095_3, 0.646_850_94, 0.020_412_8]),
            (3000.0, [1.770_840_9, 0.844_284_9, 0.272_222_8]),
            (4000.0, [1.414_116_1, 0.923_641_9, 0.533_674_24]),
            (6500.0, [1.042_552_1, 0.984_052_54, 1.035_271_5]),
            (10000.0, [0.871_384_1, 0.994_924_55, 1.430_508_9]),
            (15000.0, [0.826_295_5, 0.994_508_03, 1.566_307_7]),
        ];
        for (t, want) in cases {
            let got = blackbody_temperature_to_rgb(t);
            for (g, w) in got.iter().zip(want) {
                assert!(
                    (g - w).abs() <= 2.0 * f32::EPSILON * w.abs().max(1.0),
                    "{t}: {got:?}"
                );
            }
        }
        let mut la = Light::new("L", "POINT");
        assert_eq!(la.temperature_color(), [1.0; 3]);
        la.fields.use_temperature = true;
        la.fields.temperature = 3000.0;
        assert_eq!(la.temperature_color(), blackbody_temperature_to_rgb(3000.0));
    }

    // Dumped from Blender 5.2.4 by re-typing the factory light.
    #[test]
    fn factory_hidden_fields_match_blender() {
        let mut la = Light::new("Light", "POINT");
        la.set_factory_startup_hidden_fields();
        assert_eq!(la.fields.spot_size, 1.3089969158172607_f64 as f32);
        assert_eq!(la.fields.angle, 0.19933730363845825_f64 as f32);
        assert_eq!(la.fields.size, 0.10000000149011612_f64 as f32);
        assert_eq!(la.fields.shape, "RECTANGLE");
        let fresh = Light::new("N", "POINT");
        assert_eq!(fresh.fields.spot_size, 0.7853981852531433_f64 as f32);
    }

    // Dumped from Blender 5.2.4: `Light.area(matrix_world=Matrix.LocRotScale((1, 2, 3),
    // Euler((0.3, 0.2, 0.1)), (2, 3, 4)))`.
    #[test]
    fn area_matches_blender() {
        let m = Mat4::from_scale_rotation_translation(
            Vec3::new(2.0, 3.0, 4.0),
            Quat::from_euler(EulerRot::ZYX, 0.1, 0.2, 0.3),
            Vec3::new(1.0, 2.0, 3.0),
        );
        let mut la = Light::new("L", "POINT");
        la.fields.shadow_soft_size = 0.25;
        assert_eq!(la.area(&m), 0.785_398_2);
        la.r#type = "SPOT".into();
        assert_eq!(la.area(&m), 0.785_398_2);
        la.fields.shadow_soft_size = 0.0;
        assert_eq!(la.area(&m), 4.0);
        la.r#type = "SUN".into();
        la.fields.angle = 0.2;
        assert!((la.area(&m) - 0.031_311_352).abs() < 1e-9);
        la.fields.angle = 0.0;
        assert_eq!(la.area(&m), 1.0);
        la.r#type = "AREA".into();
        la.fields.size = 1.5;
        la.fields.size_y = 0.5;
        for (shape, want) in [
            ("SQUARE", 13.499_999),
            ("RECTANGLE", 4.499_999_5),
            ("DISK", 10.602_875),
            ("ELLIPSE", 3.534_291_5),
        ] {
            la.fields.shape = shape.into();
            assert!(
                (la.area(&m) - want).abs() < 2e-6,
                "{shape}: {}",
                la.area(&m)
            );
        }
    }
}
