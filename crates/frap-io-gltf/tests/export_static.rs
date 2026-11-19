// SPDX-License-Identifier: GPL-3.0-or-later
//! Library-level glTF export (WP 1.16): Rust-built scenes export to files that pass
//! `frap_gltf::validate_file` and the `gltf =1.4.1` validating reader, and follow the
//! io_scene_gltf2 5.2.41 rules. Expected values marked "Blender 5.2.4" were read from
//! `bpy.ops.export_scene.gltf` on the same scene (the Blender goldens proper are WP 1.23's).

mod export_support;

use export_support::{Scene, png, set_material_indices, settings, snapshot_json};
use frap_gltf::json::{AlphaMode, ComponentType, Mode};
use frap_io_gltf::export::{ExportFormat, ExportImageFormat, ExportMaterials, ExportSettings};
use frap_math::glam::Mat4;
use frap_mesh::primitives::cube;
use frap_mesh::primitives::uv_sphere::{UvSphere, uv_sphere};

fn close(a: &[f64], b: &[f64]) {
    assert_eq!(a.len(), b.len(), "{a:?} vs {b:?}");
    for (x, y) in a.iter().zip(b) {
        assert!((x - y).abs() < 2e-6, "{a:?} vs {b:?}");
    }
}

fn node<'a>(d: &'a frap_gltf::Document, name: &str) -> &'a frap_gltf::json::Node {
    d.nodes
        .iter()
        .find(|n| n.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("node {name}"))
}

fn node_index(d: &frap_gltf::Document, name: &str) -> u32 {
    d.nodes
        .iter()
        .position(|n| n.name.as_deref() == Some(name))
        .unwrap() as u32
}

#[test]
fn factory_scene_snapshot() {
    let s = Scene::factory();
    let out = s.export(&settings());
    assert!(out.reports.is_empty(), "{:?}", out.reports);
    insta::assert_snapshot!("factory_scene", snapshot_json(&out.document));
}

#[test]
#[allow(clippy::excessive_precision)] // Blender's printed doubles of float32 values
fn hierarchy_and_transforms_match_blender() {
    let mut s = Scene::empty();
    let root = s.empty_object("Root");
    {
        let o = s.ob(root);
        o.location = [1.0, 2.0, 3.0];
        o.rotation_euler = [0.3, 0.2, 0.1];
    }
    let child = s.mesh_object("Child", cube(2.0, false, [1.0; 3]));
    let leaf = s.empty_object("Leaf");
    for n in ["Object10", "object2"] {
        s.empty_object(n);
    }
    {
        // Blender 5.2.4: `child.matrix_parent_inverse = root.matrix_world.inverted()` (rows).
        let rows = [
            [0.975_170_37, 0.097_843_4, -0.198_669_34, -0.574_849_2],
            [-0.036_957_014, 0.956_425_1, 0.289_629_52, -2.744_782],
            [0.218_350_66, -0.275_095_88, 0.936_293_4, -2.477_039_1],
            [-0.0, 0.0, -0.0, 1.0],
        ];
        let o = s.ob(child);
        o.parent = Some(root);
        o.matrix_parent_inverse = Mat4::from_cols_array_2d(&rows).transpose();
        o.location = [0.5, 0.0, 0.0];
        o.rotation_mode = "QUATERNION".into();
        o.rotation_quaternion = [0.9, 0.1, 0.2, 0.3];
        o.scale = [2.0, 1.0, 0.5];
    }
    {
        let o = s.ob(leaf);
        o.parent = Some(child);
        o.rotation_mode = "AXIS_ANGLE".into();
        o.rotation_axis_angle = [0.5, 0.0, 0.0, 1.0];
        o.location = [0.0, 1.0, 0.0];
    }
    let out = s.export(&settings());
    let d = &out.document;
    // Blender 5.2.4: nodes post-order, roots in natural case-insensitive order.
    let names: Vec<_> = d.nodes.iter().map(|n| n.name.clone().unwrap()).collect();
    assert_eq!(names, ["object2", "Object10", "Leaf", "Child", "Root"]);
    assert_eq!(d.scenes[0].nodes, [0, 1, 4]);
    assert_eq!(node(d, "Root").children, [3]);
    assert_eq!(node(d, "Child").children, [2]);
    let r = node(d, "Root");
    close(&r.translation.unwrap(), &[1.0, 3.0, -2.0]);
    close(
        &r.rotation.unwrap(),
        &[
            0.143_572_211_263_561_96,
            0.034_270_808_100_700_38,
            -0.106_020_539_999_008_18,
            0.983_347_535_133_361_8,
        ],
    );
    assert_eq!((r.scale, r.mesh), (None, None));
    let c = node(d, "Child");
    close(
        &c.translation.unwrap(),
        &[
            -0.087_264_038_622_379_3,
            -2.367_863_893_508_911,
            2.763_260_841_369_629,
        ],
    );
    close(
        &c.rotation.unwrap(),
        &[
            -0.057_282_667_607_069_016,
            0.252_440_065_145_492_55,
            -0.144_555_836_915_969_85,
            0.955_037_474_632_263_2,
        ],
    );
    close(&c.scale.unwrap(), &[2.0, 0.499_999_940_395_355_2, 1.0]);
    assert_eq!(c.mesh, Some(0));
    let l = node(d, "Leaf");
    close(
        &l.translation.unwrap(),
        &[0.0, 0.0, -0.999_999_940_395_355_2],
    );
    close(
        &l.rotation.unwrap(),
        &[0.0, 0.247_404_009_103_775, 0.0, 0.968_912_422_657_013],
    );
    // The mesh datablock's name (the helper names it after its object).
    assert_eq!(d.meshes[0].name.as_deref(), Some("Child"));
    // Blender 5.2.4: POSITION min/max only, NORMAL, then UNSIGNED_SHORT indices.
    let p = &d.meshes[0].primitives[0];
    assert_eq!(
        p.attributes.keys().collect::<Vec<_>>(),
        ["POSITION", "NORMAL"]
    );
    assert_eq!(d.accessors[0].max.as_deref(), Some(&[1.0, 1.0, 1.0][..]));
    assert!(d.accessors[1].max.is_none());
    assert_eq!(d.accessors[2].component_type, ComponentType::UnsignedShort);
    assert_eq!(d.accessors[2].count, 36);
    assert_eq!(p.mode, None);
    insta::assert_snapshot!("hierarchy", snapshot_json(d));
}

#[test]
fn natural_root_order_and_flatten() {
    let mut s = Scene::empty();
    let a = s.empty_object("A");
    let b = s.empty_object("b");
    s.ob(b).parent = Some(a);
    s.ob(b).location = [0.0, 0.0, 1.0];
    for n in ["Item10", "item2", "Item1"] {
        s.empty_object(n);
    }
    let out = s.export(&settings());
    let roots: Vec<_> = out.document.scenes[0]
        .nodes
        .iter()
        .map(|i| out.document.nodes[*i as usize].name.clone().unwrap())
        .collect();
    assert_eq!(roots, ["A", "Item1", "item2", "Item10"]);
    // Flattened: the child becomes a root with its world transform.
    let flat = ExportSettings {
        export_hierarchy_flatten_objs: true,
        ..settings()
    };
    let out = s.export(&flat);
    let d = &out.document;
    assert_eq!(d.scenes[0].nodes.len(), 5);
    assert!(d.nodes.iter().all(|n| n.children.is_empty()));
    assert_eq!(node(d, "b").translation, Some([0.0, 1.0, 0.0]));
}

#[test]
fn multi_material_primitives() {
    let mut s = Scene::empty();
    let red = s.material("Red", [1.0, 0.0, 0.0, 1.0], 0.0, 0.5);
    let blue = s.material("Blue", [0.0, 0.0, 1.0, 1.0], 1.0, 0.25);
    let ob = s.mesh_object("Box", cube(2.0, true, [1.0; 3]));
    {
        let me = s.mesh_of(ob);
        me.materials = vec![Some(red), Some(blue)];
        // Index 2 is past the last slot: the add-on uses the last slot (Blue).
        set_material_indices(me, vec![0, 1, 0, 1, 2, 2]);
    }
    let out = s.export(&settings());
    let d = &out.document;
    let prims = &d.meshes[0].primitives;
    assert_eq!(prims.len(), 3);
    let mats: Vec<_> = prims
        .iter()
        .map(|p| {
            d.materials[p.material.unwrap() as usize]
                .name
                .clone()
                .unwrap()
        })
        .collect();
    assert_eq!(mats, ["Red", "Blue", "Blue"]);
    assert_eq!(d.materials.len(), 2);
    // Each face of the cube has its own 4 corners (flat, UVs): 8 vertices for 2 faces.
    for p in prims {
        assert_eq!(d.accessors[p.attributes["POSITION"] as usize].count, 8);
        assert_eq!(d.accessors[p.indices.unwrap() as usize].count, 12);
    }
    insta::assert_snapshot!("multi_material", snapshot_json(d));
    let red_m = &d.materials[0];
    let pbr = red_m.pbr_metallic_roughness.as_ref().unwrap();
    assert_eq!(pbr.base_color_factor, Some([1.0, 0.0, 0.0, 1.0]));
    assert_eq!(pbr.metallic_factor, Some(0.0));
    assert_eq!(pbr.roughness_factor, Some(0.5));
    assert_eq!(red_m.double_sided, Some(true));
    let blue_m = &d.materials[1];
    let pbr = blue_m.pbr_metallic_roughness.as_ref().unwrap();
    assert_eq!(pbr.metallic_factor, None, "metallic 1 is the glTF default");
    assert_eq!(pbr.roughness_factor, Some(0.25));

    // PLACEHOLDER keeps the split without materials; NONE merges into one primitive.
    let ph = s.export(&ExportSettings {
        export_materials: ExportMaterials::Placeholder,
        ..settings()
    });
    assert_eq!(ph.document.meshes[0].primitives.len(), 3);
    assert!(ph.document.materials.is_empty());
    assert!(
        ph.document.meshes[0]
            .primitives
            .iter()
            .all(|p| p.material.is_none())
    );
    let none = s.export(&ExportSettings {
        export_materials: ExportMaterials::None,
        ..settings()
    });
    let prims = &none.document.meshes[0].primitives;
    assert_eq!(prims.len(), 1);
    assert_eq!(
        none.document.accessors[prims[0].attributes["POSITION"] as usize].count,
        24
    );
    // VIEWPORT writes the viewport values, every factor present.
    let vp = s.export(&ExportSettings {
        export_materials: ExportMaterials::Viewport,
        ..settings()
    });
    let pbr = vp.document.materials[0]
        .pbr_metallic_roughness
        .as_ref()
        .unwrap();
    close(
        &pbr.base_color_factor.unwrap(),
        &[
            0.800_000_011_920_929,
            0.800_000_011_920_929,
            0.800_000_011_920_929,
            1.0,
        ],
    );
    assert_eq!(pbr.metallic_factor, Some(0.0));
    assert!(vp.document.materials[0].double_sided.is_none());
}

#[test]
fn shared_meshes_and_object_linked_slots() {
    let mut s = Scene::empty();
    let red = s.material("Red", [1.0, 0.0, 0.0, 1.0], 0.0, 0.5);
    let green = s.material("Green", [0.0, 1.0, 0.0, 1.0], 0.0, 0.5);
    let a = s.mesh_object("A", cube(2.0, true, [1.0; 3]));
    s.mesh_of(a).materials = vec![Some(red)];
    let b = s.linked_duplicate("B", a);
    let c = s.linked_duplicate("C", a);
    // C overrides its slot with an object-linked material.
    s.ob(c).material_slot_overrides = vec![frap_scene::id::SlotOverride {
        link: frap_scene::id::SlotLink::Object,
        material: Some(green),
    }];
    let _ = b;
    let out = s.export(&settings());
    let d = &out.document;
    let (ma, mb, mc) = (
        node(d, "A").mesh.unwrap(),
        node(d, "B").mesh.unwrap(),
        node(d, "C").mesh.unwrap(),
    );
    assert_eq!(ma, mb, "linked duplicates share one glTF mesh");
    assert_ne!(ma, mc, "other materials make another glTF mesh");
    // ... which reuses the accessors (the add-on caches primitives by mesh).
    let (pa, pc) = (
        &d.meshes[ma as usize].primitives[0],
        &d.meshes[mc as usize].primitives[0],
    );
    assert_eq!(pa.attributes, pc.attributes);
    assert_eq!(pa.indices, pc.indices);
    assert_ne!(pa.material, pc.material);
    assert_eq!(out.meshes, 2);
}

#[test]
fn flat_and_smooth_normals_split_like_blender() {
    let mut s = Scene::empty();
    let flat = s.mesh_object("Flat", uv_sphere(&UvSphere::default()));
    let mut smooth_mesh = uv_sphere(&UvSphere::default());
    smooth_mesh.shade_smooth();
    s.mesh_object("Smooth", smooth_mesh);
    let _ = flat;
    let mut st = settings();
    st.export_texcoords = false;
    let out = s.export(&st);
    let d = &out.document;
    let count = |name: &str| {
        let m = node(d, name).mesh.unwrap() as usize;
        let p = &d.meshes[m].primitives[0];
        (
            d.accessors[p.attributes["POSITION"] as usize].count,
            d.accessors[p.indices.unwrap() as usize].count,
        )
    };
    // 32 × 16 UV sphere: 480 faces (448 quads, 32 + 32 triangles), 2880 indices; flat faces
    // split every corner (1984 = 448 × 4 + 64 × 3), smooth ones share vertices (482).
    assert_eq!(count("Flat"), (1984, 2880));
    assert_eq!(count("Smooth"), (482, 2880));
    // With UVs, the seam splits the smooth sphere further.
    let out = s.export(&settings());
    let d = &out.document;
    let m = node(d, "Smooth").mesh.unwrap() as usize;
    let p = &d.meshes[m].primitives[0];
    assert!(p.attributes.contains_key("TEXCOORD_0"));
    assert_eq!(d.accessors[p.attributes["POSITION"] as usize].count, 559);
}

#[test]
fn normals_are_unit_and_yup() {
    let s = Scene::factory();
    let out = s.export(&settings());
    let g = gltf::Gltf::from_slice(&out.bytes).unwrap();
    let blob = g.blob.clone().unwrap();
    let prim = g
        .document
        .meshes()
        .next()
        .unwrap()
        .primitives()
        .next()
        .unwrap();
    let r = prim.reader(|_| Some(blob.as_slice()));
    let normals: Vec<[f32; 3]> = r.read_normals().unwrap().collect();
    assert_eq!(normals.len(), 24);
    for n in &normals {
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        assert!((len - 1.0).abs() < 1e-6);
        // Axis-aligned cube normals; Blender +Z faces are glTF +Y.
        assert_eq!(n.iter().filter(|c| c.abs() == 1.0).count(), 1);
        assert!(n.iter().all(|c| c.to_bits() != (-0.0f32).to_bits()));
    }
    // V flipped: Blender's factory cube UVs (0.375..0.875) become 1 - v.
    let uvs: Vec<[f32; 2]> = r.read_tex_coords(0).unwrap().into_f32().collect();
    assert!(uvs.iter().all(|uv| (0.0..=1.0).contains(&uv[1])));
    assert!(uvs.iter().any(|uv| uv[1] == 1.0 - 0.25 || uv[1] == 0.75));
}

#[test]
fn base_color_textures_glb() {
    let mut s = Scene::empty();
    let bytes = png(2, 2);
    let img = s.packed_image("Checker.png", bytes.clone(), "image/png");
    // A readable JPEG: an image Blender cannot load has size (0, 0) and no texture (WP 3.5).
    let jpeg_bytes = frap_imbuf::encode(
        &frap_imbuf::DecodedImage {
            width: 2,
            height: 2,
            channels: 3,
            pixels: frap_imbuf::PixelData::U8(vec![200; 12]),
        },
        &frap_imbuf::EncodeFormat::Jpeg { quality: 90 },
    )
    .unwrap();
    let jpg = s.packed_image("Photo", jpeg_bytes.clone(), "image/jpeg");
    let generated = s.generated_image("Blank", [0.5, 0.25, 1.0, 1.0]);
    let m1 = s.material("Textured", [1.0; 4], 0.0, 0.5);
    s.base_color_texture(m1, img, false);
    let m2 = s.material("Alpha", [1.0; 4], 0.0, 0.5);
    s.base_color_texture(m2, img, true);
    let m3 = s.material("Generated", [1.0; 4], 0.0, 0.5);
    s.base_color_texture(m3, generated, false);
    let m4 = s.material("Jpeg", [1.0; 4], 0.0, 0.5);
    s.base_color_texture(m4, jpg, false);
    let ob = s.mesh_object("Box", cube(2.0, true, [1.0; 3]));
    {
        let me = s.mesh_of(ob);
        me.materials = vec![Some(m1), Some(m2), Some(m3), Some(m4)];
        set_material_indices(me, vec![0, 1, 2, 3, 0, 0]);
    }
    let out = s.export(&settings());
    let d = &out.document;
    assert_eq!(d.materials.len(), 4);
    // The PNG is copied byte for byte into the BIN (passthrough).
    let tex_of = |m: usize| {
        d.materials[m]
            .pbr_metallic_roughness
            .as_ref()
            .unwrap()
            .base_color_texture
            .clone()
            .unwrap()
    };
    let t0 = tex_of(0);
    assert_eq!(t0.tex_coord, None, "TEXCOORD_0 is the default");
    let image = &d.images[d.textures[t0.index as usize].source.unwrap() as usize];
    assert_eq!(image.mime_type.as_deref(), Some("image/png"));
    assert_eq!(image.name.as_deref(), Some("Checker"));
    let g = frap_gltf::read_glb(&out.bytes).unwrap();
    let view = &d.buffer_views[image.buffer_view.unwrap() as usize];
    let bin = g.bin.unwrap();
    let at = view.byte_offset.unwrap_or(0) as usize;
    assert_eq!(&bin[at..at + view.byte_length as usize], &bytes[..]);
    assert!(view.target.is_none());
    // The base colour factor is omitted with a texture (white).
    assert!(
        d.materials[0]
            .pbr_metallic_roughness
            .as_ref()
            .unwrap()
            .base_color_factor
            .is_none()
    );
    // Same image with alpha: same PNG image and sampler, but a texture of its own (Blender
    // writes one texture per material usage), BLEND.
    assert_ne!(tex_of(1).index, t0.index);
    assert_eq!(
        d.textures[tex_of(1).index as usize],
        d.textures[t0.index as usize]
    );
    assert_eq!(d.materials[1].alpha_mode, Some(AlphaMode::Blend));
    // Generated image encoded to PNG, its own image.
    let gi = &d.images[d.textures[tex_of(2).index as usize].source.unwrap() as usize];
    assert_eq!(gi.name.as_deref(), Some("Blank"));
    let gv = &d.buffer_views[gi.buffer_view.unwrap() as usize];
    let at = gv.byte_offset.unwrap_or(0) as usize;
    let decoded = frap_imbuf::decode(&bin[at..at + gv.byte_length as usize]).unwrap();
    assert_eq!((decoded.width, decoded.height), (4, 2));
    // JPEG source stays JPEG (bytes copied).
    let ji = &d.images[d.textures[tex_of(3).index as usize].source.unwrap() as usize];
    assert_eq!(ji.mime_type.as_deref(), Some("image/jpeg"));
    let jv = &d.buffer_views[ji.buffer_view.unwrap() as usize];
    let at = jv.byte_offset.unwrap_or(0) as usize;
    assert_eq!(&bin[at..at + jv.byte_length as usize], &jpeg_bytes[..]);
    // One shared sampler (LINEAR / LINEAR_MIPMAP_LINEAR, REPEAT omitted).
    assert_eq!(d.samplers.len(), 1);
    assert_eq!(u32::from(d.samplers[0].mag_filter.unwrap()), 9729);
    assert_eq!(u32::from(d.samplers[0].min_filter.unwrap()), 9987);
    assert!(d.samplers[0].wrap_s.is_none());
    // No textures with export_image_format='NONE'.
    let none = s.export(&ExportSettings {
        export_image_format: ExportImageFormat::None,
        ..settings()
    });
    assert!(none.document.images.is_empty() && none.document.textures.is_empty());
}

/// Blender 5.2.4 (`bpy.ops.export_scene.gltf`, GLTF_SEPARATE) on: materials Textured (Color),
/// Alpha (Color + Alpha) and Again (Color into Base Color and Emission Color) all using one
/// Checker.png; a cube using the three, then a plane using Textured. Result: textures
/// `[{0,0}] * 4`, one image, one sampler; Textured 0, Alpha 1, Again emissive 2 then base 3,
/// the plane's material shares Textured's texture.
#[test]
fn one_texture_per_material_usage_like_blender() {
    let mut s = Scene::empty();
    let img = s.packed_image("Checker.png", png(2, 2), "image/png");
    let m1 = s.material("Textured", [1.0; 4], 0.0, 0.5);
    s.base_color_texture(m1, img, false);
    let m2 = s.material("Alpha", [1.0; 4], 0.0, 0.5);
    s.base_color_texture(m2, img, true);
    let m3 = s.material("Again", [1.0; 4], 0.0, 0.5);
    s.base_color_texture(m3, img, false);
    s.link_emission_from_texture(m3, 1.0);
    let ob = s.mesh_object("Cube", cube(2.0, true, [1.0; 3]));
    {
        let me = s.mesh_of(ob);
        me.materials = vec![Some(m1), Some(m2), Some(m3)];
        set_material_indices(me, vec![0, 1, 2, 0, 0, 0]);
    }
    let plane = s.mesh_object("Plane", cube(1.0, true, [1.0; 3]));
    s.mesh_of(plane).materials = vec![Some(m1)];
    let out = s.export(&settings());
    let d = &out.document;
    assert_eq!(d.textures.len(), 4);
    assert!(
        d.textures
            .iter()
            .all(|t| t.sampler == Some(0) && t.source == Some(0))
    );
    assert_eq!((d.images.len(), d.samplers.len()), (1, 1));
    let base = |name: &str| {
        let m = d
            .materials
            .iter()
            .find(|m| m.name.as_deref() == Some(name))
            .unwrap();
        let b = m.pbr_metallic_roughness.as_ref().unwrap();
        (
            b.base_color_texture.as_ref().unwrap().index,
            m.emissive_texture.as_ref().map(|t| t.index),
        )
    };
    assert_eq!(base("Textured"), (0, None));
    assert_eq!(base("Alpha"), (1, None));
    assert_eq!(base("Again"), (3, Some(2)));
    assert_eq!(d.materials.len(), 3, "the plane reuses Textured");
}

#[test]
fn gltf_separate_and_embedded() {
    let mut s = Scene::factory();
    let img = s.packed_image("Checker.png", png(2, 2), "image/png");
    let mat = s.main.materials.find("Material").unwrap();
    s.base_color_texture(mat, img, false);
    let sep = s.export(&ExportSettings {
        export_format: ExportFormat::GltfSeparate,
        file_stem: "part".into(),
        export_texture_dir: "tex dir".into(),
        ..settings()
    });
    let text = String::from_utf8(sep.bytes.clone()).unwrap();
    assert!(text.ends_with("}\n"));
    assert!(
        text.contains("\n\t\"asset\":{"),
        "tab-indented, no space after ':'"
    );
    let d = &sep.document;
    assert_eq!(d.buffers[0].uri.as_deref(), Some("part.bin"));
    assert_eq!(d.images[0].uri.as_deref(), Some("tex%20dir/Checker.png"));
    assert!(d.images[0].buffer_view.is_none());
    let paths: Vec<_> = sep.files.iter().map(|(p, _)| p.as_str()).collect();
    assert_eq!(paths, ["part.bin", "tex dir/Checker.png"]);
    // Embedded: one self-contained .gltf.
    let emb = s.export(&ExportSettings {
        export_format: ExportFormat::GltfEmbedded,
        ..settings()
    });
    assert!(emb.files.is_empty());
    let uri = emb.document.buffers[0].uri.as_deref().unwrap();
    assert!(uri.starts_with("data:application/octet-stream;base64,"));
    assert!(emb.document.images[0].buffer_view.is_some());
}

#[test]
fn filters_and_reparenting() {
    let mut s = Scene::factory();
    let parent = s.empty_object("Parent");
    let kid = s.mesh_object("Kid", cube(1.0, false, [1.0; 3]));
    s.ob(parent).location = [0.0, 0.0, 2.0];
    s.ob(kid).parent = Some(parent);
    s.ob(kid).location = [1.0, 0.0, 0.0];
    // Default: cameras and lights are not exported; Kid is Parent's child.
    let out = s.export(&settings());
    let names: Vec<_> = out
        .document
        .nodes
        .iter()
        .map(|n| n.name.clone().unwrap())
        .collect();
    assert_eq!(names, ["Cube", "Kid", "Parent"]);
    // Only Kid selected: it becomes a root with its world transform.
    let sc = s.win.scene;
    for b in &mut s.main.scenes.get_mut(sc, &mut s.log).unwrap().view_layers[0].bases {
        b.flag = if b.object == kid {
            frap_scene::BaseFlag::SELECTED
        } else {
            frap_scene::BaseFlag::empty()
        };
    }
    let out = s.export(&ExportSettings {
        use_selection: true,
        ..settings()
    });
    let d = &out.document;
    assert_eq!(d.nodes.len(), 1);
    assert_eq!(d.scenes[0].nodes, [node_index(d, "Kid")]);
    assert_eq!(node(d, "Kid").translation, Some([1.0, 2.0, 0.0]));
    // use_renderable drops hide_render objects; their kept children move up.
    s.ob(parent).hide_render = true;
    let out = s.export(&ExportSettings {
        use_renderable: true,
        ..settings()
    });
    let d = &out.document;
    assert!(d.nodes.iter().all(|n| n.name.as_deref() != Some("Parent")));
    assert!(d.scenes[0].nodes.contains(&node_index(d, "Kid")));
    s.ob(parent).hide_render = false;
    // glTF_not_exported always excludes its objects.
    let special = s.collection("glTF_not_exported");
    s.main
        .collections
        .get_mut(special, &mut s.log)
        .unwrap()
        .objects
        .push(kid);
    let out = s.export(&settings());
    assert!(
        out.document
            .nodes
            .iter()
            .all(|n| n.name.as_deref() != Some("Kid"))
    );
    // `collection`: only that collection's objects; the scene is named after it.
    let out = s.export(&ExportSettings {
        collection: "glTF_not_exported".into(),
        ..settings()
    });
    assert!(out.document.nodes.is_empty());
    assert_eq!(
        out.document.scenes[0].name.as_deref(),
        Some("glTF_not_exported")
    );
    let out = s.export(&ExportSettings {
        collection: "Collection".into(),
        ..settings()
    });
    assert_eq!(out.document.nodes.len(), 2, "Cube and Parent");
}

#[test]
fn visibility_filter() {
    let mut s = Scene::factory();
    let cube_ob = s.main.objects.find("Cube").unwrap();
    let out = s.export(&ExportSettings {
        use_visible: true,
        ..settings()
    });
    assert_eq!(out.document.nodes.len(), 1);
    s.ob(cube_ob).hide_viewport = true;
    let out = s.export(&ExportSettings {
        use_visible: true,
        ..settings()
    });
    assert!(out.document.nodes.is_empty());
}

#[test]
fn omission_rules() {
    // Nothing exported: asset, scene and an empty scene object; no buffers.
    let s = Scene::empty();
    let out = s.export(&settings());
    let d = &out.document;
    assert!(d.nodes.is_empty() && d.buffers.is_empty() && d.accessors.is_empty());
    assert_eq!(d.scene, Some(0));
    let json = String::from_utf8(d.to_vec(&frap_gltf::WriteOpts::default())).unwrap();
    assert_eq!(
        json,
        r#"{"asset":{"generator":"frap test","version":"2.0"},"scene":0,"scenes":[{"name":"Scene"}]}"#
    );
    // A GLB without binary data has no BIN chunk.
    assert_eq!(out.bytes.len(), 12 + 8 + json.len().div_ceil(4) * 4);
    // A mesh without faces is omitted with a warning; its node stays.
    let mut s = Scene::empty();
    s.mesh_object("Nothing", frap_mesh::Mesh::new("Nothing"));
    let out = s.export(&settings());
    assert_eq!(out.document.nodes.len(), 1);
    assert!(out.document.nodes[0].mesh.is_none());
    assert!(out.reports[0].message.contains("no primitives"));
    // Copyright goes into the asset.
    let out = s.export(&ExportSettings {
        export_copyright: "CC0".into(),
        ..settings()
    });
    assert_eq!(out.document.asset.copyright.as_deref(), Some("CC0"));
}

#[test]
fn z_up_export() {
    let s = Scene::factory();
    let out = s.export(&ExportSettings {
        export_yup: false,
        ..settings()
    });
    let g = gltf::Gltf::from_slice(&out.bytes).unwrap();
    let blob = g.blob.clone().unwrap();
    let prim = g
        .document
        .meshes()
        .next()
        .unwrap()
        .primitives()
        .next()
        .unwrap();
    let r = prim.reader(|_| Some(blob.as_slice()));
    let pos: Vec<[f32; 3]> = r.read_positions().unwrap().collect();
    // Z-up: vertex 0 of the factory cube stays (1, 1, 1).
    assert_eq!(pos[0], [1.0, 1.0, 1.0]);
}

#[test]
fn large_meshes_use_unsigned_int_indices() {
    let mut s = Scene::empty();
    let big = UvSphere {
        segments: 300,
        ring_count: 250,
        ..UvSphere::default()
    };
    s.mesh_object("Big", uv_sphere(&big));
    let mut st = settings();
    st.export_texcoords = false;
    let out = s.export(&st);
    let d = &out.document;
    let p = &d.meshes[0].primitives[0];
    assert!(d.accessors[p.attributes["POSITION"] as usize].count >= 65535);
    assert_eq!(
        d.accessors[p.indices.unwrap() as usize].component_type,
        ComponentType::UnsignedInt
    );
    assert_eq!(p.mode.unwrap_or(Mode::Triangles), Mode::Triangles);
}
