// SPDX-License-Identifier: GPL-3.0-or-later
//! RNA bindings of `Mesh` and its element structs (ARCHITECTURE §6.1, §9.4; WP 1.27).
//!
//! `MeshVertex`, `MeshEdge`, `MeshPolygon`, `MeshLoop`, `MeshLoopTriangle`, `MeshNormalValue`,
//! the attribute layers and their typed `data[i]`, `MeshUVLoopLayer`/`MeshUVLoop` and
//! `MeshLoopColorLayer`/`MeshLoopColor` are proxies over the struct-of-arrays columns
//! ([`soa`](mod@soa)); every element collection overrides `foreach_get`/`foreach_set` with column
//! copies ([`bulk`]). Writes go through `Mesh`'s `&mut` methods, which clear the runtime
//! caches.
//!
//! The bindings are hand-written (`rna_struct!` has no named, refined or nested SoA
//! collections) and declare themselves through [`frap_rna::testing::RnaBindings`], so
//! `check_bindings::<Mesh>()` checks them like generated ones ([`bind`]).
//!
//! The Python-only RNA functions `Mesh.from_pydata/shade_smooth/shade_flat` and the
//! collection functions (`vertices.add`, `attributes.new`, `uv_layers.new`, …) are global
//! registrations (WP 1.22, decision log W0-22); this crate provides the Rust `Mesh` API.
//!
//! Not bound (no storage on frap's `Mesh`, or they need the host): `is_editmode`,
//! `animation_data`, `texture_mesh`, `texco_mesh`, `uv_layer_clone[_index]`,
//! `uv_layer_stencil[_index]`, `skin_vertices`, the remesh/symmetry/paint-mask settings,
//! `MeshUVLoopLayer.active_clone`, and the collection-struct properties
//! (`uv_layers.active`, `attributes.active_color`, …), which the frozen `RnaColl` cannot
//! expose. Reading or writing one of them gives `NotImplementedError` (`RNA_NOT_IMPLEMENTED`,
//! [`soa::unbound`]), never Blender's "has no attribute" wording.

mod attribute;
mod attribute_value;
mod bind;
mod bulk;
mod color;
mod columns;
mod mesh_edge;
mod mesh_loop;
mod mesh_loop_triangle;
mod mesh_polygon;
mod mesh_uv_loop_layer;
mod mesh_vertex;
mod soa;
#[cfg(test)]
mod tests;

use frap_rna::testing::{BindingDecl, RnaBindings};
use frap_rna::traits::{
    FromRnaValue, IdVecColl, IdVecCollMut, RnaStruct, ToRnaValue, read_only, wrong_access,
};
use frap_rna::*;
use frap_spec_rna::{self as spec, MESH};

use self::attribute::{AttrFilter, Attributes};
use self::bind::{other, readonly, soa, value};
use self::mesh_edge::Edges;
use self::mesh_loop::Loops;
use self::mesh_loop_triangle::{LoopTrianglePolygons, LoopTriangles, NormalDomain, NormalValues};
use self::mesh_polygon::Polygons;
use self::mesh_uv_loop_layer::{LoopColorLayers, UvLayers};
use self::mesh_vertex::Vertices;
use self::soa::{coll_field, coll_mut_guard};
use crate::mesh::{CUSTOM_NORMAL, Mesh};

/// The refined RNA struct of an attribute (`FloatAttribute`, `ByteColorAttribute`, …), for
/// the pointers `attributes.new()` returns.
pub fn attribute_srna(data: &crate::AttrData) -> &'static StructDef {
    attribute::refined_srna(data)
}

/// `Mesh` bindings (`ID` base properties first, as WP 1.10 binds them for every datablock).
static BINDINGS: [BindingDecl; 40] = [
    other!("name", SetHook),
    other!("name_full", Hook),
    other!("users", Hook),
    readonly!("id_type", &'static str),
    readonly!("session_uid", i32),
    readonly!("is_evaluated", bool),
    value!("use_fake_user", bool),
    readonly!("is_embedded_data", bool),
    readonly!("is_linked_packed", bool),
    readonly!("is_missing", bool),
    readonly!("is_editable", bool),
    readonly!("is_library_indirect", bool),
    other!("library", Id),
    soa!("vertices", spec::MESH_VERTEX, &mesh_vertex::BINDINGS),
    soa!("edges", spec::MESH_EDGE, &mesh_edge::BINDINGS),
    soa!("loops", spec::MESH_LOOP, &mesh_loop::BINDINGS),
    soa!("polygons", spec::MESH_POLYGON, &mesh_polygon::BINDINGS),
    soa!(
        "loop_triangles",
        spec::MESH_LOOP_TRIANGLE,
        &mesh_loop_triangle::BINDINGS
    ),
    soa!(
        "loop_triangle_polygons",
        spec::READ_ONLY_INTEGER,
        &mesh_loop_triangle::INTEGER_BINDINGS
    ),
    soa!(
        "vertex_normals",
        spec::MESH_NORMAL_VALUE,
        &mesh_loop_triangle::NORMAL_BINDINGS
    ),
    soa!(
        "polygon_normals",
        spec::MESH_NORMAL_VALUE,
        &mesh_loop_triangle::NORMAL_BINDINGS
    ),
    soa!(
        "corner_normals",
        spec::MESH_NORMAL_VALUE,
        &mesh_loop_triangle::NORMAL_BINDINGS
    ),
    soa!(
        "uv_layers",
        spec::MESH_UV_LOOP_LAYER,
        &mesh_uv_loop_layer::UV_LAYER_BINDINGS
    ),
    soa!(
        "vertex_colors",
        spec::MESH_LOOP_COLOR_LAYER,
        &mesh_uv_loop_layer::COLOR_LAYER_BINDINGS
    ),
    attribute::ATTRIBUTES,
    attribute::COLOR_ATTRIBUTES,
    attribute::MATERIALS,
    readonly!("normals_domain", &'static str),
    readonly!("has_custom_normals", bool),
    other!("shape_keys", Id),
    value!("use_auto_texspace", bool),
    value!("auto_texspace", bool),
    value!("texspace_location", [f32; 3]),
    value!("texspace_size", [f32; 3]),
    readonly!("total_vert_sel", i32),
    readonly!("total_edge_sel", i32),
    readonly!("total_face_sel", i32),
    // The glTF add-on's variant properties (WP 7.5): registered only while the add-on
    // preference `KHR_materials_variants_ui` is on, which only the host sees, so the
    // `frap-scene` hooks (`hooks/mesh_gltf2_*.rs`) serve them (their `HOOK_DECLS` live there).
    other!("gltf2_variant_mesh_data", HookColl),
    other!("gltf2_variant_default_materials", HookColl),
    other!("gltf2_variant_pointer", Hook),
];

// `ID.name`, `ID.name_full` and `ID.users` go through the `ID` hooks every datablock shares
// (`#[rna_hook(srna = "ID", …)]`, WP 1.10 in frap-scene, which can see `Main`). Their
// `HOOK_DECLS` entries are emitted by frap-scene's `ID` bindings (the completeness test keys on
// `(srna, prop)`, so one declaration covers every ID type); declaring them here too would make
// `frap-ops-all/tests/registry.rs` fail whenever frap-mesh is linked without those hooks.

impl RnaStruct for Mesh {
    fn struct_def() -> &'static StructDef {
        &MESH
    }
}

impl RnaBindings for Mesh {
    fn bindings() -> &'static [BindingDecl] {
        &BINDINGS
    }
}

// Ported from blender/source/blender/blenkernel/intern/mesh.cc (BKE_mesh_texspace_calc).
/// `(texspace_location, texspace_size)`: from the bounds while `use_auto_texspace` is on
/// (`(-1, -1, -1)..(1, 1, 1)` without vertices; zero sizes become 1, tiny ones ±1e-5).
fn texspace(m: &Mesh) -> ([f32; 3], [f32; 3]) {
    if !m.texspace.auto {
        return (m.texspace.location, m.texspace.size);
    }
    let [min, max] = m.bounds().unwrap_or([[-1.0; 3], [1.0; 3]]);
    let mut loc = [0.0; 3];
    let mut size = [0.0; 3];
    for a in 0..3 {
        loc[a] = 0.5 * (min[a] + max[a]);
        size[a] = (max[a] - min[a]) / 2.0;
        if size[a] == 0.0 {
            size[a] = 1.0;
        } else if size[a] > 0.0 && size[a] < 0.00001 {
            size[a] = 0.00001;
        } else if size[a] < 0.0 && size[a] > -0.00001 {
            size[a] = -0.00001;
        }
    }
    (loc, size)
}

fn tag(prop: &PropDef) -> MathTag {
    MathTag::for_subtype(prop.subtype)
}

impl Rna for Mesh {
    fn srna(&self) -> &'static StructDef {
        &MESH
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        let h = &self.header;
        let none = MathTag::None;
        let v = match prop.identifier {
            "name" => Value::Str(h.name.clone()),
            "name_full" | "users" => return Ok(Field::NeedsHook),
            "id_type" => "MESH".to_rna_value(prop, none),
            "session_uid" => (h.session_uid as i32).to_rna_value(prop, none),
            "is_evaluated" | "is_missing" | "is_library_indirect" => Value::Bool(false),
            "use_fake_user" => Value::Bool(h.use_fake_user),
            "is_embedded_data" => Value::Bool(h.is_embedded_data),
            "is_linked_packed" => Value::Bool(h.is_linked_packed),
            "is_editable" => Value::Bool(h.library.is_none()),
            "library" => return Ok(Field::Id(h.library.map(|l| l.any()))),
            "vertices" => return Ok(coll_field::<Vertices>(self, ())),
            "edges" => return Ok(coll_field::<Edges>(self, ())),
            "loops" => return Ok(coll_field::<Loops>(self, ())),
            "polygons" => return Ok(coll_field::<Polygons>(self, ())),
            "loop_triangles" => return Ok(coll_field::<LoopTriangles>(self, ())),
            "loop_triangle_polygons" => return Ok(coll_field::<LoopTrianglePolygons>(self, ())),
            "vertex_normals" => return Ok(coll_field::<NormalValues>(self, NormalDomain::Vert)),
            "polygon_normals" => return Ok(coll_field::<NormalValues>(self, NormalDomain::Face)),
            "corner_normals" => {
                return Ok(coll_field::<NormalValues>(self, NormalDomain::Corner));
            }
            "uv_layers" => return Ok(coll_field::<UvLayers>(self, ())),
            "vertex_colors" => return Ok(coll_field::<LoopColorLayers>(self, ())),
            "attributes" => return Ok(coll_field::<Attributes>(self, AttrFilter::All)),
            "color_attributes" => return Ok(coll_field::<Attributes>(self, AttrFilter::Color)),
            "materials" => {
                return Ok(Field::Coll(CollRef::Owned(Box::new(IdVecColl::new(
                    &self.materials,
                    &spec::MATERIAL,
                )))));
            }
            "normals_domain" => self.normals_domain().identifier().to_rna_value(prop, none),
            "has_custom_normals" => Value::Bool(self.attributes().get(CUSTOM_NORMAL).is_some()),
            "shape_keys" => return Ok(Field::Id(self.shape_keys.map(|k| k.any()))),
            "use_auto_texspace" | "auto_texspace" => Value::Bool(self.texspace.auto),
            "texspace_location" => texspace(self).0.to_rna_value(prop, tag(prop)),
            "texspace_size" => texspace(self).1.to_rna_value(prop, tag(prop)),
            // Edit-mode selection counts (`em->bm->totvertsel`), 0 outside Edit Mode (verified):
            // the edit BMesh lives in the session, whose hook reads it (WP 2.2).
            "total_vert_sel" | "total_edge_sel" | "total_face_sel" => return Ok(Field::NeedsHook),
            // The glTF add-on's variant data exists only while its preference is on (WP 7.5).
            "gltf2_variant_mesh_data"
            | "gltf2_variant_default_materials"
            | "gltf2_variant_pointer" => return Ok(Field::NeedsHook),
            _ => return Err(soa::unbound(&MESH, prop.identifier)),
        };
        Ok(Field::Value(v))
    }
}

impl RnaMut for Mesh {
    fn set(&mut self, prop: &'static PropDef, v: Value) -> Result<SetOutcome, RnaError> {
        match prop.identifier {
            "name" => return Ok(SetOutcome::NeedsHook), // unique_name + re-sort (§9.5)
            "name_full" | "users" => return Ok(SetOutcome::NeedsHook),
            "use_fake_user" => self.header.use_fake_user = bool::from_rna_value(prop, v)?,
            "use_auto_texspace" | "auto_texspace" => {
                let auto = bool::from_rna_value(prop, v)?;
                if self.texspace.auto && !auto {
                    // Blender keeps the last automatic texture space (`BKE_mesh_texspace_ensure`
                    // stored it in `me->texspace_*`); verified on 5.2.4: a size-4 cube keeps
                    // `(0, 0, 0)` / `(2, 2, 2)` after `use_auto_texspace = False`.
                    let (location, size) = texspace(self);
                    self.texspace.location = location;
                    self.texspace.size = size;
                }
                self.texspace.auto = auto;
            }
            // Editable only while the texture space is not automatic (verified:
            // `attribute "texspace_location" from "Mesh" is read-only`).
            "texspace_location" | "texspace_size" if self.texspace.auto => {
                return Err(read_only(&MESH, prop.identifier));
            }
            "texspace_location" => self.texspace.location = <[f32; 3]>::from_rna_value(prop, v)?,
            "texspace_size" => self.texspace.size = <[f32; 3]>::from_rna_value(prop, v)?,
            "gltf2_variant_pointer" => return Ok(SetOutcome::NeedsHook),
            p if BINDINGS.iter().any(|b| b.prop == p) => return Err(read_only(&MESH, p)),
            p => return Err(soa::unbound(&MESH, p)),
        }
        Ok(SetOutcome::Done)
    }
    fn struct_mut(&mut self, prop: &'static PropDef) -> Result<RnaMutGuard<'_>, RnaError> {
        Err(wrong_access(&MESH, prop.identifier, "a struct"))
    }
    fn coll_mut(&mut self, prop: &'static PropDef) -> Result<RnaCollMutGuard<'_>, RnaError> {
        Ok(match prop.identifier {
            "vertices" => coll_mut_guard::<Vertices>(self, ()),
            "edges" => coll_mut_guard::<Edges>(self, ()),
            "loops" => coll_mut_guard::<Loops>(self, ()),
            "polygons" => coll_mut_guard::<Polygons>(self, ()),
            "loop_triangles" => coll_mut_guard::<LoopTriangles>(self, ()),
            "loop_triangle_polygons" => coll_mut_guard::<LoopTrianglePolygons>(self, ()),
            "vertex_normals" => coll_mut_guard::<NormalValues>(self, NormalDomain::Vert),
            "polygon_normals" => coll_mut_guard::<NormalValues>(self, NormalDomain::Face),
            "corner_normals" => coll_mut_guard::<NormalValues>(self, NormalDomain::Corner),
            "uv_layers" => coll_mut_guard::<UvLayers>(self, ()),
            "vertex_colors" => coll_mut_guard::<LoopColorLayers>(self, ()),
            "attributes" => coll_mut_guard::<Attributes>(self, AttrFilter::All),
            "color_attributes" => coll_mut_guard::<Attributes>(self, AttrFilter::Color),
            "materials" => Box::new(IdVecCollMut::new(&mut self.materials, &spec::MATERIAL)),
            p => return Err(wrong_access(&MESH, p, "a collection")),
        })
    }
}

impl IdType for Mesh {
    type Kind = MeshId;
    fn header(&self) -> &IdHeader {
        &self.header
    }
    fn header_mut(&mut self) -> &mut IdHeader {
        &mut self.header
    }
    fn for_each_ref(&self, f: &mut dyn FnMut(IdAny)) {
        for m in self.materials.iter().flatten() {
            f(m.any());
        }
        if let Some(k) = self.shape_keys {
            f(k.any());
        }
        // The glTF add-on's variant materials (M7 W0).
        for m in self.gltf2_variants.materials() {
            f(m.any());
        }
    }
    fn remap_ref(&mut self, from: IdAny, to: Option<IdAny>) {
        for m in self.materials.iter_mut() {
            if m.map(|r| r.any()) == Some(from) {
                *m = to.and_then(IdRef::from_any);
            }
        }
        if self.shape_keys.map(|r| r.any()) == Some(from) {
            self.shape_keys = to.and_then(IdRef::from_any);
        }
        if let Some(m) = IdRef::<MaterialId>::from_any(from) {
            self.gltf2_variants
                .remap_material(m, to.and_then(IdRef::from_any));
        }
    }
}
