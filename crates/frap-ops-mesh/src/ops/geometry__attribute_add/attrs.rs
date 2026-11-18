// SPDX-License-Identifier: GPL-3.0-or-later
//! The attribute API the `geometry.*` operators and `mesh.uv_texture_*` / `mesh.attribute_set`
//! share: one view over the attributes of a mesh in Object Mode (`Mesh` attributes) or in Edit
//! Mode (the edit BMesh's custom-data layers), plus the mesh-level names that refer to them
//! (active/default colour, active/render UV map, active attribute).
//!
//! Ported from blender/source/blender/blenkernel/intern/attribute.cc (Blender 5.2.4,
//! GPL-2.0-or-later): `BKE_attribute_calc_unique_name`, `BKE_attribute_new` /
//! `mesh_attribute_valid`, `BKE_attribute_remove` (colour and UV names re-resolved by clamped
//! index, the UV pin layer removed with its map), `BKE_attributes_active_name_get`,
//! `BKE_attributes_active_index_validate`, and
//! blender/source/blender/blenkernel/intern/attribute_access.cc
//! (`allow_procedural_attribute_access`).
//!
//! **The active attribute.** Blender stores an *index* (`Mesh.attributes_active_index`) into
//! the attribute list: in Object Mode every attribute of the mesh in storage order (internal
//! ones included), in Edit Mode the BMesh layers of the point, edge, face and corner domains
//! in that order (layers sorted by type, `.pn.<uv>` pin layers included, the topology and flag
//! attributes left out). frap stores the active attribute's name
//! (`frap_mesh::Mesh::attributes_active`) and reproduces the index arithmetic of each operator
//! with [`index_of`] / [`name_at`] / [`validate_index`]: removing an attribute before the active
//! one moves the activity to the next attribute, as in Blender.

use std::sync::Arc;

use frap_bmesh::convert::{stored_in_bmesh_builtin, uv_pin_name};
use frap_bmesh::{BMesh, CdLayer, CustomData};
use frap_mesh::{AttrData, Attribute, Domain, Mesh};
use smol_str::SmolStr;

/// Byte limit of attribute names (`MAX_CUSTOMDATA_LAYER_NAME - 1`).
pub(crate) const MAX_NAME: usize = 67;

/// The attribute domains of a mesh in Blender's order (`AttrDomain` values 0–3).
pub(crate) const DOMAINS: [Domain; 4] = [Domain::Point, Domain::Edge, Domain::Face, Domain::Corner];

/// One attribute as the index-based API lists it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Layer {
    /// Name.
    pub name: SmolStr,
    /// Domain.
    pub domain: Domain,
    /// `data_type` identifier (`FLOAT`, `FLOAT_COLOR`, …).
    pub data_type: &'static str,
}

/// `bke::mesh::is_color_attribute`: FLOAT_COLOR / BYTE_COLOR on POINT or CORNER.
pub(crate) fn is_color(domain: Domain, data_type: &str) -> bool {
    matches!(data_type, "FLOAT_COLOR" | "BYTE_COLOR")
        && matches!(domain, Domain::Point | Domain::Corner)
}

/// `bke::mesh::is_uv_map`: FLOAT2 on CORNER (internal `.`-prefixed layers are never UV maps).
pub(crate) fn is_uv(name: &str, domain: Domain, data_type: &str) -> bool {
    domain == Domain::Corner && data_type == "FLOAT2" && !name.starts_with('.')
}

/// `bke::allow_procedural_attribute_access`.
pub(crate) fn allow_procedural(name: &str) -> bool {
    !(name.starts_with(".corner")
        || name.starts_with(".edge")
        || name.starts_with(".select")
        || name.starts_with(".sculpt")
        || name.starts_with(".hide")
        || name.starts_with(".uv")
        || name == ".reference_index"
        || name.starts_with(".pn."))
}

/// The custom data of a BMesh domain.
pub(crate) fn cd(bm: &BMesh, d: Domain) -> &CustomData {
    match d {
        Domain::Point => &bm.vdata,
        Domain::Edge => &bm.edata,
        Domain::Face => &bm.pdata,
        Domain::Corner => &bm.ldata,
    }
}

/// The custom data of a BMesh domain, mutably.
pub(crate) fn cd_mut(bm: &mut BMesh, d: Domain) -> &mut CustomData {
    match d {
        Domain::Point => &mut bm.vdata,
        Domain::Edge => &mut bm.edata,
        Domain::Face => &mut bm.pdata,
        Domain::Corner => &mut bm.ldata,
    }
}

/// The slot capacity of a BMesh domain (every column holds one value per slot).
pub(crate) fn slots(bm: &BMesh, d: Domain) -> usize {
    match d {
        Domain::Point => bm.verts.capacity(),
        Domain::Edge => bm.edges.capacity(),
        Domain::Face => bm.faces.capacity(),
        Domain::Corner => bm.loops.capacity(),
    }
}

/// An attribute container: a mesh in Object Mode or an edit BMesh.
pub(crate) trait Store {
    /// Whether this is an edit BMesh.
    fn is_edit(&self) -> bool;
    /// The attributes in index order (module docs).
    fn layers(&self) -> Vec<Layer>;
    /// Whether a new attribute may not take `name` (vertex groups are checked by the caller).
    fn taken(&self, name: &str) -> bool;
    /// The column of `name` (by element index in Object Mode, by slot in Edit Mode).
    fn column(&self, name: &str) -> Option<(Domain, AttrData)>;
    /// Appends (Object Mode) or inserts by type (Edit Mode) a new attribute; `data` has one
    /// value per element (Object Mode) or per slot (Edit Mode).
    fn insert(&mut self, name: SmolStr, domain: Domain, data: AttrData);
    /// Replaces the values of an existing attribute (same domain).
    fn replace(&mut self, name: &str, data: AttrData);
    /// Removes an attribute; its domain and type.
    fn take(&mut self, name: &str) -> Option<Layer>;
    /// The number of values a new column of `domain` needs.
    fn column_len(&self, domain: Domain) -> usize;
    /// `object_active_color_init` → `fill_mesh_color`: sets the colour attribute `name` to
    /// `color` on every face corner (CORNER) or on every vertex used by a face (POINT); a byte
    /// colour is sRGB-encoded.
    fn fill_color(&mut self, name: &str, color: [f32; 4]);
}

/// Writes `color` into `data` at `indices` (FLOAT_COLOR as is, BYTE_COLOR encoded).
fn fill_indices(data: &mut AttrData, indices: impl Iterator<Item = usize>, color: [f32; 4]) {
    match data {
        AttrData::FloatColor(v) => {
            let v = Arc::make_mut(v);
            for i in indices {
                v[i] = color;
            }
        }
        AttrData::ByteColor(v) => {
            let v = Arc::make_mut(v);
            let b = encode_byte_color(color);
            for i in indices {
                v[i] = b;
            }
        }
        _ => {}
    }
}

impl Store for Mesh {
    fn is_edit(&self) -> bool {
        false
    }
    fn layers(&self) -> Vec<Layer> {
        self.attributes()
            .iter()
            .map(|a| Layer {
                name: a.name.clone(),
                domain: a.domain,
                data_type: a.data.data_type(),
            })
            .collect()
    }
    fn taken(&self, name: &str) -> bool {
        self.attributes().get(name).is_some()
    }
    fn column(&self, name: &str) -> Option<(Domain, AttrData)> {
        self.attributes()
            .get(name)
            .map(|a| (a.domain, a.data.clone()))
    }
    fn insert(&mut self, name: SmolStr, domain: Domain, data: AttrData) {
        self.attributes_mut()
            .insert(Attribute { name, domain, data });
    }
    fn replace(&mut self, name: &str, data: AttrData) {
        if let Some(a) = self.attributes_mut().get_mut(name) {
            a.data = data;
        }
    }
    fn take(&mut self, name: &str) -> Option<Layer> {
        self.attributes_mut().remove(name).map(|a| Layer {
            name: a.name,
            domain: a.domain,
            data_type: a.data.data_type(),
        })
    }
    fn column_len(&self, domain: Domain) -> usize {
        self.domain_size(domain)
    }
    fn fill_color(&mut self, name: &str, color: [f32; 4]) {
        let corner_verts = self.corner_verts().to_vec();
        let corners = 0..self.corners_num();
        let Some(a) = self.attributes_mut().get_mut(name) else {
            return;
        };
        match a.domain {
            Domain::Corner => fill_indices(&mut a.data, corners, color),
            Domain::Point => {
                fill_indices(&mut a.data, corner_verts.iter().map(|&v| v as usize), color)
            }
            _ => {}
        }
    }
}

impl Store for BMesh {
    fn is_edit(&self) -> bool {
        true
    }
    fn layers(&self) -> Vec<Layer> {
        let mut out = Vec::new();
        for d in DOMAINS {
            for l in &cd(self, d).layers {
                out.push(Layer {
                    name: l.name.clone(),
                    domain: d,
                    data_type: l.data.data_type(),
                });
            }
        }
        out
    }
    fn taken(&self, name: &str) -> bool {
        stored_in_bmesh_builtin(name) || DOMAINS.iter().any(|&d| cd(self, d).find(name).is_some())
    }
    fn column(&self, name: &str) -> Option<(Domain, AttrData)> {
        DOMAINS
            .iter()
            .find_map(|&d| cd(self, d).get(name).map(|l| (d, l.data.clone())))
    }
    fn insert(&mut self, name: SmolStr, domain: Domain, data: AttrData) {
        cd_mut(self, domain).add_layer(CdLayer { name, data });
    }
    fn replace(&mut self, name: &str, data: AttrData) {
        for d in DOMAINS {
            if let Some(i) = cd(self, d).find(name) {
                cd_mut(self, d).layers[i].data = data;
                return;
            }
        }
    }
    fn take(&mut self, name: &str) -> Option<Layer> {
        for d in DOMAINS {
            if let Some(l) = cd_mut(self, d).remove_layer(name) {
                return Some(Layer {
                    name: l.name,
                    domain: d,
                    data_type: l.data.data_type(),
                });
            }
        }
        None
    }
    fn column_len(&self, domain: Domain) -> usize {
        slots(self, domain)
    }
    fn fill_color(&mut self, name: &str, color: [f32; 4]) {
        let Some((domain, _)) = self.column(name) else {
            return;
        };
        let mut idx = Vec::new();
        for f in self.face_ids() {
            for l in self.face_loops(f) {
                idx.push(match domain {
                    Domain::Corner => l.slot(),
                    _ => self.lp(l).v.slot(),
                });
            }
        }
        if !matches!(domain, Domain::Corner | Domain::Point) {
            return;
        }
        let i = cd(self, domain).find(name).expect("found above");
        fill_indices(
            &mut cd_mut(self, domain).layers[i].data,
            idx.into_iter(),
            color,
        );
    }
}

/// The mesh-level names that refer to attributes (stored on the original mesh; the UV ones
/// also on the edit BMesh's corner data while editing).
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Names {
    /// `uv_layers.active`.
    pub uv_active: Option<SmolStr>,
    /// `uv_layers` active-render (`default_uv_map_name`).
    pub uv_render: Option<SmolStr>,
    /// `color_attributes.active_color_name`.
    pub color_active: Option<SmolStr>,
    /// `color_attributes.default_color_name`.
    pub color_default: Option<SmolStr>,
    /// `attributes.active`.
    pub active: Option<SmolStr>,
}

impl Names {
    /// The names of `me`.
    pub(crate) fn of(me: &Mesh) -> Names {
        Names {
            uv_active: me.uv_active.clone(),
            uv_render: me.uv_active_render.clone(),
            color_active: me.color_active.clone(),
            color_default: me.color_default.clone(),
            active: me.attributes_active.clone(),
        }
    }
    /// Writes the names to `me`.
    pub(crate) fn write(&self, me: &mut Mesh) {
        me.uv_active.clone_from(&self.uv_active);
        me.uv_active_render.clone_from(&self.uv_render);
        me.color_active.clone_from(&self.color_active);
        me.color_default.clone_from(&self.color_default);
        me.attributes_active.clone_from(&self.active);
    }
    /// Writes the UV names to the edit BMesh (which `BMesh::to_mesh` reads on leaving Edit
    /// Mode).
    pub(crate) fn write_bm(&self, bm: &mut BMesh) {
        bm.ldata.uv_active.clone_from(&self.uv_active);
        bm.ldata.uv_render.clone_from(&self.uv_render);
    }
}

/// `BKE_attribute_calc_unique_name`: `requested` (empty → `Attribute`) made unique among the
/// attributes (Edit Mode: the BMesh layers and the attributes stored as BMesh topology/flags)
/// and the vertex groups.
pub(crate) fn unique_name(store: &dyn Store, vgroups: &[SmolStr], requested: &str) -> SmolStr {
    let taken = |n: &str| store.taken(n) || vgroups.iter().any(|g| g == n);
    frap_rna::naming::unique_subname(&taken, requested, MAX_NAME, "Attribute")
}

/// Position of `name` in `list`, -1 when absent.
pub(crate) fn index_of(list: &[Layer], name: Option<&str>) -> i32 {
    name.and_then(|n| list.iter().position(|l| l.name == n))
        .map_or(-1, |i| i as i32)
}

/// `BKE_attributes_active_name_get` for an index: `None` out of range; in Edit Mode an index
/// past the end reads as 0 and a non-procedural layer (`.pn.UVMap`) as none.
pub(crate) fn name_at(list: &[Layer], index: i32, edit: bool) -> Option<SmolStr> {
    if index < 0 {
        return None;
    }
    let mut i = index as usize;
    if edit {
        if i > list.len() {
            i = 0;
        }
        return list
            .get(i)
            .filter(|l| allow_procedural(&l.name))
            .map(|l| l.name.clone());
    }
    list.get(i).map(|l| l.name.clone())
}

/// `BKE_attributes_active_index_validate`: the nearest procedural attribute at or below
/// `index`, else above it, else none (-1).
pub(crate) fn validate_index(list: &[Layer], index: i32) -> i32 {
    if index < 0 || list.is_empty() {
        return -1;
    }
    let ok = |i: i32| {
        list.get(i as usize)
            .is_some_and(|l| allow_procedural(&l.name))
    };
    let mut i = index;
    while i >= 0 {
        if (i as usize) < list.len() && ok(i) {
            return i;
        }
        i -= 1;
    }
    let mut i = index + 1;
    while (i as usize) < list.len() {
        if ok(i) {
            return i;
        }
        i += 1;
    }
    -1
}

/// The active attribute (`BKE_attributes_active_name_get`): the stored name when it is in the
/// list (and, in Edit Mode, a procedural layer).
pub(crate) fn active_name(store: &dyn Store, names: &Names) -> Option<SmolStr> {
    let list = store.layers();
    let i = index_of(&list, names.active.as_deref());
    name_at(&list, i, store.is_edit())
}

/// Index of `name` among the colour attributes, -1 when absent.
fn color_index(list: &[Layer], name: Option<&str>) -> i32 {
    let colors: Vec<&Layer> = list
        .iter()
        .filter(|l| is_color(l.domain, l.data_type))
        .collect();
    name.and_then(|n| colors.iter().position(|l| l.name == n))
        .map_or(-1, |i| i as i32)
}

/// `color_name_from_index(color_clamp_index(index))`.
fn color_clamped(list: &[Layer], index: i32) -> Option<SmolStr> {
    let colors: Vec<&Layer> = list
        .iter()
        .filter(|l| is_color(l.domain, l.data_type))
        .collect();
    let i = index.min(colors.len() as i32 - 1);
    (i >= 0).then(|| colors[i as usize].name.clone())
}

/// Index of `name` among the UV maps, -1 when absent.
fn uv_index(list: &[Layer], name: Option<&str>) -> i32 {
    let uvs: Vec<&Layer> = list
        .iter()
        .filter(|l| is_uv(&l.name, l.domain, l.data_type))
        .collect();
    name.and_then(|n| uvs.iter().position(|l| l.name == n))
        .map_or(-1, |i| i as i32)
}

/// `uv_name_from_index(uv_clamp_index(index))`.
fn uv_clamped(list: &[Layer], index: i32) -> Option<SmolStr> {
    let uvs: Vec<&Layer> = list
        .iter()
        .filter(|l| is_uv(&l.name, l.domain, l.data_type))
        .collect();
    let i = index.min(uvs.len() as i32 - 1);
    (i >= 0).then(|| uvs[i as usize].name.clone())
}

/// Attributes that cannot be removed (`BKE_mesh_attribute_required`).
pub(crate) fn required(name: &str) -> bool {
    matches!(
        name,
        "position" | ".edge_verts" | ".corner_vert" | ".corner_edge"
    )
}

/// `BKE_attribute_remove`: removes `name`; the active/default colour and active/render UV map
/// that named it move to the attribute now at their (clamped) index; a UV map's pin layer
/// goes with it. The active attribute is not touched (callers adjust it). `Err` carries
/// Blender's report.
pub(crate) fn remove(store: &mut dyn Store, names: &mut Names, name: &str) -> Result<bool, String> {
    if name.is_empty() {
        return Err("The attribute name must not be empty".into());
    }
    if required(name) {
        return Err(format!(
            "Attribute '{name}' is required and cannot be removed"
        ));
    }
    let before = store.layers();
    let Some(layer) = before.iter().find(|l| l.name == name).cloned() else {
        return Ok(false);
    };
    let is_active_color = names.color_active.as_deref() == Some(name);
    let is_default_color = names.color_default.as_deref() == Some(name);
    let is_active_uv = names.uv_active.as_deref() == Some(name);
    let is_default_uv = names.uv_render.as_deref() == Some(name);
    let active_color_index = color_index(&before, names.color_active.as_deref());
    let default_color_index = color_index(&before, names.color_default.as_deref());
    let active_uv_index = uv_index(&before, names.uv_active.as_deref());
    let default_uv_index = uv_index(&before, names.uv_render.as_deref());
    store.take(name);
    let after = store.layers();
    if is_active_color {
        names.color_active = color_clamped(&after, active_color_index);
    }
    if is_default_color {
        names.color_default = color_clamped(&after, default_color_index);
    }
    if is_active_uv {
        names.uv_active = uv_clamped(&after, active_uv_index);
    }
    if is_default_uv {
        names.uv_render = uv_clamped(&after, default_uv_index);
    }
    if layer.domain == Domain::Corner && layer.data_type == "FLOAT2" {
        store.take(&uv_pin_name(name));
    }
    Ok(true)
}

/// A column of `len` values of the CustomData default of `data_type` (`CD_SET_DEFAULT`:
/// white colours, identity quaternions and matrices, zero otherwise).
pub(crate) fn default_column(data_type: &str, len: usize) -> Option<AttrData> {
    frap_mesh::default_column(data_type, len)
}

/// Adds a default-valued attribute to `store` under the exact name `name`.
pub(crate) fn add_default(
    store: &mut dyn Store,
    name: SmolStr,
    domain: Domain,
    data_type: &str,
) -> Option<()> {
    let data = default_column(data_type, store.column_len(domain))?;
    store.insert(name, domain, data);
    Some(())
}

/// `BM_uv_map_attr_pin_ensure_for_all_layers`: a pin layer for every UV map of the edit
/// BMesh that lacks one.
pub(crate) fn pin_ensure_all(bm: &mut BMesh) {
    let n = bm.loops.capacity();
    for uv in bm.ldata.float2_names() {
        if uv.starts_with('.') {
            continue;
        }
        let pin = uv_pin_name(&uv);
        if bm.ldata.find(&pin).is_none() {
            bm.ldata.add_layer(CdLayer {
                name: pin,
                data: AttrData::Bool(Arc::new(vec![false; n])),
            });
        }
    }
}

/// `linearrgb_to_srgb` (scalar path).
fn linear_to_srgb(c: f32) -> f32 {
    if c < 0.003_130_8 {
        if c < 0.0 { 0.0 } else { c * 12.92 }
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// `unit_float_to_uchar_clamp`.
fn unit_float_to_uchar_clamp(v: f32) -> u8 {
    if v <= 0.0 {
        0
    } else if v > 1.0 - 0.5 / 255.0 {
        255
    } else {
        (255.0 * v + 0.5) as u8
    }
}

/// `color::encode`: a scene-linear colour as an sRGB-encoded byte colour (alpha linear).
pub(crate) fn encode_byte_color(c: [f32; 4]) -> [u8; 4] {
    [
        unit_float_to_uchar_clamp(linear_to_srgb(c[0])),
        unit_float_to_uchar_clamp(linear_to_srgb(c[1])),
        unit_float_to_uchar_clamp(linear_to_srgb(c[2])),
        unit_float_to_uchar_clamp(c[3]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layer(name: &str) -> Layer {
        Layer {
            name: name.into(),
            domain: Domain::Point,
            data_type: "FLOAT",
        }
    }

    #[test]
    fn validate_prefers_lower_procedural_index() {
        let list: Vec<Layer> = ["foo", ".select_vert", "bar", ".corner_vert"]
            .into_iter()
            .map(layer)
            .collect();
        assert_eq!(validate_index(&list, 1), 0);
        assert_eq!(validate_index(&list, 3), 2);
        assert_eq!(validate_index(&list, 7), 2);
        let internal: Vec<Layer> = [".select_vert", "x"].into_iter().map(layer).collect();
        assert_eq!(validate_index(&internal, 0), 1);
        assert_eq!(validate_index(&[], 0), -1);
        assert_eq!(validate_index(&list, -1), -1);
    }

    #[test]
    fn name_at_edit_rules() {
        let list: Vec<Layer> = ["foo", "UVMap", ".pn.UVMap"]
            .into_iter()
            .map(layer)
            .collect();
        assert_eq!(name_at(&list, 2, true), None);
        assert_eq!(name_at(&list, 9, true).as_deref(), Some("foo"));
        assert_eq!(name_at(&list, 9, false), None);
        assert_eq!(name_at(&list, 1, false).as_deref(), Some("UVMap"));
    }

    #[test]
    fn byte_color_encoding() {
        assert_eq!(encode_byte_color([0.0, 0.0, 0.0, 1.0]), [0, 0, 0, 255]);
        assert_eq!(
            encode_byte_color([1.0, 0.5, 0.2140411, 0.5]),
            [255, 188, 127, 128]
        );
    }

    #[test]
    fn remove_moves_color_and_uv_names() {
        let mut me = Mesh::new("m");
        me.add_attribute("A", "FLOAT_COLOR", "POINT").expect("A");
        me.add_attribute("B", "BYTE_COLOR", "CORNER").expect("B");
        let mut names = Names::of(&me);
        names.color_default = Some("B".into());
        assert_eq!(names.color_active.as_deref(), Some("A"));
        assert_eq!(remove(&mut me, &mut names, "A"), Ok(true));
        assert_eq!(names.color_active.as_deref(), Some("B"));
        assert_eq!(remove(&mut me, &mut names, "B"), Ok(true));
        assert_eq!(
            (names.color_active.clone(), names.color_default.clone()),
            (None, None)
        );
        assert!(remove(&mut me, &mut names, "position").is_err());
        assert_eq!(remove(&mut me, &mut names, "nope"), Ok(false));
    }
}
