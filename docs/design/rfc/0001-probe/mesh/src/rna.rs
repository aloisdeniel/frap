// SPDX-License-Identifier: GPL-3.0-or-later
//! Hand-written bindings standing in for `rna_struct!` output (§9.4, §9.6 step 4) for `Mesh`,
//! `MeshVertex` (SoA proxies) and `IDMaterials` (a collection of nullable ID pointers).

use core::ops::Range;

use frap_probe_rna::*;
use frap_probe_spec as spec;
use smallvec::SmallVec;
use smol_str::SmolStr;

use crate::Mesh;

fn id_field(srna: &'static StructDef, h: &IdHeader, prop: &'static PropDef) -> Result<Field<'static>, RnaError> {
    match prop.identifier {
        "name" => Ok(Field::Value(Value::Str(h.name.clone()))),
        "is_embedded_data" => Ok(Field::Value(Value::Bool(h.is_embedded_data))),
        "use_fake_user" => Ok(Field::Value(Value::Bool(h.use_fake_user))),
        _ => Err(unbound(srna, prop.identifier)),
    }
}

/// Shared by every ID binding in the probe (what `rna_struct!` would emit for `ID`).
pub fn id_get(srna: &'static StructDef, h: &IdHeader, prop: &'static PropDef) -> Result<Field<'static>, RnaError> {
    id_field(srna, h, prop)
}

/// `name` itself is a hook in §9.6 (`unique_name` + re-sort): bindings answer `NeedsHook`.
pub fn id_set(srna: &'static StructDef, h: &mut IdHeader, prop: &'static PropDef, v: Value) -> Result<SetOutcome, RnaError> {
    match (prop.identifier, v) {
        ("name", _) => Ok(SetOutcome::NeedsHook),
        ("use_fake_user", Value::Bool(b)) => {
            h.use_fake_user = b;
            Ok(SetOutcome::Done)
        }
        (id, _) if prop.flags.contains(PropFlags::READONLY) => Err(readonly(srna, id)),
        (id, _) => Err(unbound(srna, id)),
    }
}

// ------------------------------------------------------------------------------- Mesh

impl Rna for Mesh {
    fn srna(&self) -> &'static StructDef {
        &spec::MESH
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        match prop.identifier {
            "vertices" => Ok(Field::Coll(CollRef::Owned(Box::new(MeshVertices { mesh: self })))),
            "materials" => Ok(Field::Coll(CollRef::Owned(Box::new(IdMaterials { mats: &self.materials })))),
            _ => id_get(self.srna(), &self.header, prop),
        }
    }
}

impl RnaMut for Mesh {
    fn set(&mut self, prop: &'static PropDef, v: Value) -> Result<SetOutcome, RnaError> {
        let srna = self.srna();
        id_set(srna, &mut self.header, prop, v)
    }
    fn struct_mut(&mut self, prop: &'static PropDef) -> Result<RnaMutGuard<'_>, RnaError> {
        Err(unbound(self.srna(), prop.identifier))
    }
    fn coll_mut(&mut self, prop: &'static PropDef) -> Result<RnaCollMutGuard<'_>, RnaError> {
        match prop.identifier {
            "vertices" => Ok(Box::new(MeshVerticesMut { mesh: self })),
            _ => Err(unbound(self.srna(), prop.identifier)),
        }
    }
}

// ------------------------------------------------------------------- MeshVertex proxies

/// Read proxy: `MeshVertexRef { mesh: &Mesh, i }` (§9.4).
pub struct MeshVertexRef<'a> {
    pub mesh: &'a Mesh,
    pub i: usize,
}
/// Write proxy: `MeshVertexMut { mesh: &'a mut Mesh, i }` (§9.4), writing through Mesh's `&mut`
/// methods so caches are invalidated (§6.1).
pub struct MeshVertexMut<'a> {
    pub mesh: &'a mut Mesh,
    pub i: usize,
}

fn vertex_get(mesh: &Mesh, i: usize, prop: &'static PropDef) -> Result<Field<'static>, RnaError> {
    match prop.identifier {
        "co" => Ok(Field::Value(Value::float3(mesh.positions()[i], math_tag(prop)))),
        "index" => Ok(Field::Value(Value::Int(i as i64))),
        "select" => Ok(Field::Value(Value::Bool(mesh.point_bool(".select_vert", i)))),
        "hide" => Ok(Field::Value(Value::Bool(mesh.point_bool(".hide_vert", i)))),
        "normal" => Ok(Field::Value(Value::float3(mesh.vert_normals()[i], math_tag(prop)))),
        _ => Err(unbound(&spec::MESH_VERTEX, prop.identifier)),
    }
}

impl Rna for MeshVertexRef<'_> {
    fn srna(&self) -> &'static StructDef {
        &spec::MESH_VERTEX
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        vertex_get(self.mesh, self.i, prop)
    }
}
impl Rna for MeshVertexMut<'_> {
    fn srna(&self) -> &'static StructDef {
        &spec::MESH_VERTEX
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        vertex_get(self.mesh, self.i, prop)
    }
}
impl RnaMut for MeshVertexMut<'_> {
    fn set(&mut self, prop: &'static PropDef, v: Value) -> Result<SetOutcome, RnaError> {
        match (prop.identifier, &v) {
            ("co", _) => {
                self.mesh.positions_mut()[self.i] = f32_array::<3>(&v)?;
                Ok(SetOutcome::Done)
            }
            ("select", Value::Bool(b)) => {
                self.mesh.set_point_bool(".select_vert", self.i, *b);
                Ok(SetOutcome::Done)
            }
            ("hide", Value::Bool(b)) => {
                self.mesh.set_point_bool(".hide_vert", self.i, *b);
                Ok(SetOutcome::Done)
            }
            (id, _) if prop.flags.contains(PropFlags::READONLY) => Err(readonly(self.srna(), id)),
            (id, _) => Err(unbound(self.srna(), id)),
        }
    }
    fn struct_mut(&mut self, prop: &'static PropDef) -> Result<RnaMutGuard<'_>, RnaError> {
        Err(unbound(self.srna(), prop.identifier))
    }
    fn coll_mut(&mut self, prop: &'static PropDef) -> Result<RnaCollMutGuard<'_>, RnaError> {
        Err(unbound(self.srna(), prop.identifier))
    }
}

// ----------------------------------------------------------------- vertices collection

pub struct MeshVertices<'a> {
    pub mesh: &'a Mesh,
}
pub struct MeshVerticesMut<'a> {
    pub mesh: &'a mut Mesh,
}

fn verts_foreach_get(mesh: &Mesh, prop: &'static PropDef, range: Range<usize>) -> Result<Bulk, RnaError> {
    let n = mesh.verts_len();
    if range.end > n || range.start > range.end {
        return Err(RnaError::new(
            PyExc::RuntimeError,
            codes::RNA_INDEX_RANGE,
            "internal error setting the array",
        ));
    }
    match prop.identifier {
        "co" => Ok(Bulk::F32(mesh.positions()[range].iter().flatten().copied().collect())),
        "select" => Ok(Bulk::Bool(range.map(|i| mesh.point_bool(".select_vert", i)).collect())),
        "index" => Ok(Bulk::I32(range.map(|i| i as i32).collect())),
        id => Err(unbound(&spec::MESH_VERTEX, id)),
    }
}

impl RnaColl for MeshVertices<'_> {
    fn item_type(&self) -> &'static StructDef {
        &spec::MESH_VERTEX
    }
    fn len(&self) -> usize {
        self.mesh.verts_len()
    }
    fn key_of(&self, _i: usize) -> Option<SmolStr> {
        None
    }
    fn find(&self, _key: &str) -> Option<usize> {
        None
    }
    fn item(&self, i: usize) -> Result<StructRef<'_>, RnaError> {
        Ok(StructRef::Owned(Box::new(MeshVertexRef { mesh: self.mesh, i })))
    }
    /// Contract form: everything goes through one `Value::FloatArray` (SmallVec<[f32; 16]>, flat,
    /// shape lost: `ArrayShape` has no `[N, 3]`).
    fn foreach_get(&self, prop: &'static PropDef, range: Range<usize>) -> Result<Value, RnaError> {
        match verts_foreach_get(self.mesh, prop, range)? {
            Bulk::F32(v) => Ok(Value::FloatArray {
                data: SmallVec::from_vec(v),
                shape: ArrayShape::Dynamic,
                tag: MathTag::None,
            }),
            Bulk::Bool(v) => Ok(Value::BoolArray(SmallVec::from_vec(v))),
            Bulk::I32(v) => Ok(Value::IntArray(v.into_iter().map(i64::from).collect())),
        }
    }
    fn foreach_get_bulk(&self, prop: &'static PropDef, range: Range<usize>) -> Result<Bulk, RnaError> {
        verts_foreach_get(self.mesh, prop, range)
    }
}

impl RnaColl for MeshVerticesMut<'_> {
    fn item_type(&self) -> &'static StructDef {
        &spec::MESH_VERTEX
    }
    fn len(&self) -> usize {
        self.mesh.verts_len()
    }
    fn key_of(&self, _i: usize) -> Option<SmolStr> {
        None
    }
    fn find(&self, _key: &str) -> Option<usize> {
        None
    }
    fn item(&self, i: usize) -> Result<StructRef<'_>, RnaError> {
        Ok(StructRef::Owned(Box::new(MeshVertexRef { mesh: self.mesh, i })))
    }
}

fn length_mismatch() -> RnaError {
    // Verified 5.2.4 wording: RuntimeError "internal error setting the array" (+ an ERROR report
    // "Array length mismatch (got 5, expected more)").
    RnaError::new(PyExc::RuntimeError, codes::RNA_LENGTH, "internal error setting the array")
}

impl RnaCollMut for MeshVerticesMut<'_> {
    fn item_mut(&mut self, i: usize) -> Result<RnaMutGuard<'_>, RnaError> {
        if i >= self.mesh.verts_len() {
            return Err(RnaError::new(
                PyExc::IndexError,
                codes::RNA_INDEX_RANGE,
                format!("bpy_prop_collection[index]: index {i} out of range, size {}", self.mesh.verts_len()),
            ));
        }
        Ok(Box::new(MeshVertexMut { mesh: &mut *self.mesh, i }))
    }
    /// Contract form `&[Value]`: one `Value` per scalar.
    fn foreach_set(&mut self, prop: &'static PropDef, flat: &[Value]) -> Result<(), RnaError> {
        let mut f = Vec::with_capacity(flat.len());
        for v in flat {
            match v {
                Value::Float(x) => f.push(*x),
                Value::Int(x) => f.push(*x as f32),
                _ => return Err(bad_value(prop.identifier)),
            }
        }
        self.foreach_set_bulk(prop, &Bulk::F32(f))
    }
    fn foreach_set_bulk(&mut self, prop: &'static PropDef, data: &Bulk) -> Result<(), RnaError> {
        match (prop.identifier, data) {
            ("co", Bulk::F32(v)) => {
                if v.len() != self.mesh.verts_len() * 3 {
                    return Err(length_mismatch());
                }
                let pos = self.mesh.positions_mut();
                for (p, c) in pos.iter_mut().zip(v.chunks_exact(3)) {
                    *p = [c[0], c[1], c[2]];
                }
                Ok(())
            }
            (id, _) => Err(unbound(&spec::MESH_VERTEX, id)),
        }
    }
}

// --------------------------------------------------- IDMaterials (nullable ID pointers)

/// `mesh.materials`: a collection whose items are ID pointers and may be `None` (verified).
/// `RnaColl::item` (contract) cannot express that; `item_any` (F-TR-4) can.
/// `key_of`/`find` need ID names, which `frap-mesh` cannot see (L4 < L6): the resolver answers
/// name lookups for ID-item collections through `IdNames` (F-TR-4).
pub struct IdMaterials<'a> {
    pub mats: &'a [Option<IdRef<MaterialId>>],
}

impl RnaColl for IdMaterials<'_> {
    fn item_type(&self) -> &'static StructDef {
        &spec::MATERIAL
    }
    fn len(&self) -> usize {
        self.mats.len()
    }
    fn key_of(&self, _i: usize) -> Option<SmolStr> {
        None
    }
    fn find(&self, _key: &str) -> Option<usize> {
        None
    }
    fn item(&self, _i: usize) -> Result<StructRef<'_>, RnaError> {
        Err(RnaError::new(
            PyExc::RuntimeError,
            codes::INTERNAL,
            "contract gap: an ID item is not a StructRef (RFC 0001 F-TR-4)",
        ))
    }
    fn item_any(&self, i: usize) -> Result<Item<'_>, RnaError> {
        Ok(Item::Id(self.mats[i].map(IdRef::any)))
    }
    fn item_seg(&self, i: usize) -> Seg {
        Seg::Index(i as i64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_cube;

    fn get_value(r: &dyn Rna, p: &'static PropDef) -> Value {
        match r.get(p).unwrap() {
            Field::Value(v) => v,
            _ => panic!("not a value"),
        }
    }

    #[test]
    fn vertex_get_set_through_collection_guards() {
        let mut me = test_cube("Cube");
        // read path: Mesh -> CollRef -> StructRef::Owned(MeshVertexRef)
        {
            let f = me.get(spec::MESH_VERTICES).unwrap();
            let Field::Coll(c) = f else { panic!() };
            assert_eq!(c.len(), 8);
            let v3 = c.item(3).unwrap();
            assert_eq!(v3.srna().identifier, "MeshVertex");
            assert_eq!(get_value(&*v3, spec::MESH_VERTEX_CO).as_f32_slice().unwrap(), &[-1.0, 1.0, 1.0]);
            assert_eq!(get_value(&*v3, spec::MESH_VERTEX_INDEX), Value::Int(3));
        }
        // normal is cached, then invalidated by a write through the proxy
        let n_before = me.vert_normals()[7];
        assert!(me.vert_normals_cached());
        {
            let mut coll = me.coll_mut(spec::MESH_VERTICES).unwrap();
            let mut v7 = coll.item_mut(7).unwrap();
            assert_eq!(
                v7.set(spec::MESH_VERTEX_CO, Value::float3([3.0, 3.0, 3.0], MathTag::Vector))
                    .unwrap(),
                SetOutcome::Done
            );
            assert_eq!(v7.set(spec::MESH_VERTEX_SELECT, Value::Bool(true)).unwrap(), SetOutcome::Done);
            let e = v7.set(spec::MESH_VERTEX_INDEX, Value::Int(1)).unwrap_err();
            assert_eq!(e.message, "bpy_struct: attribute \"index\" from \"MeshVertex\" is read-only");
        }
        assert!(!me.vert_normals_cached(), "write through MeshVertexMut must clear runtime caches");
        assert_eq!(me.positions()[7], [3.0, 3.0, 3.0]);
        assert!(me.point_bool(".select_vert", 7));
        assert_ne!(me.vert_normals()[7], [0.0; 3]);
        let _ = n_before;
    }

    #[test]
    fn hide_creates_attribute_and_keeps_it() {
        let mut me = test_cube("Cube");
        assert!(me.attributes().get(".hide_vert").is_none());
        let mut coll = me.coll_mut(spec::MESH_VERTICES).unwrap();
        coll.item_mut(0).unwrap().set(spec::MESH_VERTEX_HIDE, Value::Bool(true)).unwrap();
        coll.item_mut(0).unwrap().set(spec::MESH_VERTEX_HIDE, Value::Bool(false)).unwrap();
        drop(coll);
        assert!(
            me.attributes().get(".hide_vert").is_some(),
            "Blender 5.2 keeps .hide_vert after unhide (verified)"
        );
    }

    #[test]
    fn foreach_bulk_and_contract_forms() {
        let mut me = test_cube("Cube");
        let Field::Coll(c) = me.get(spec::MESH_VERTICES).unwrap() else {
            panic!()
        };
        let bulk = c.foreach_get_bulk(spec::MESH_VERTEX_CO, 0..8).unwrap();
        assert_eq!(bulk.len(), 24);
        let v = c.foreach_get(spec::MESH_VERTEX_CO, 2..4).unwrap();
        assert_eq!(v.as_f32_slice().unwrap(), &[-1.0, 1.0, -1.0, -1.0, 1.0, 1.0]);
        drop(c);
        let mut cm = me.coll_mut(spec::MESH_VERTICES).unwrap();
        let flat: Vec<Value> = (0..24).map(|i| Value::Float(i as f32)).collect();
        cm.foreach_set(spec::MESH_VERTEX_CO, &flat).unwrap();
        let e = cm.foreach_set_bulk(spec::MESH_VERTEX_CO, &Bulk::F32(vec![0.0; 5])).unwrap_err();
        assert_eq!(
            (e.exception, e.message.as_str()),
            (PyExc::RuntimeError, "internal error setting the array")
        );
        drop(cm);
        assert_eq!(me.positions()[1], [3.0, 4.0, 5.0]);
    }

    #[test]
    fn mesh_is_send_sync_and_clone_is_cold() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Mesh>();
        assert_send_sync::<MeshVertexMut<'static>>();
        let me = test_cube("Cube");
        let _ = me.vert_normals();
        let c = me.clone();
        assert!(me.vert_normals_cached() && !c.vert_normals_cached());
    }
}
