// SPDX-License-Identifier: GPL-3.0-or-later
//! `Material` with its embedded node tree (§5.1 embedded IDs, §5.6).

use frap_probe_mesh::rna::{id_get, id_set};
use frap_probe_nodes::NodeTree;
use frap_probe_rna::*;
use frap_probe_spec as spec;

#[derive(Clone, Debug)]
pub struct Material {
    pub header: IdHeader,
    pub node_tree: NodeTree,
    pub diffuse_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
}

impl Material {
    pub fn new(name: &str) -> Material {
        Material {
            header: IdHeader::named(name),
            node_tree: NodeTree::new_material_tree(),
            diffuse_color: [0.8, 0.8, 0.8, 1.0],
            metallic: 0.0,
            roughness: 0.4,
        }
    }
}

impl IdType for Material {
    type Kind = MaterialId;
    fn header(&self) -> &IdHeader {
        &self.header
    }
    fn header_mut(&mut self) -> &mut IdHeader {
        &mut self.header
    }
    fn for_each_ref(&self, _f: &mut dyn FnMut(IdAny)) {}
    fn remap_ref(&mut self, _from: IdAny, _to: Option<IdAny>) {}
    fn embedded(&self) -> &'static [EmbeddedSlot] {
        &[EmbeddedSlot::NodeTree]
    }
}

impl Rna for Material {
    fn srna(&self) -> &'static StructDef {
        &spec::MATERIAL
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        match prop.identifier {
            // Embedded ID: neither `Field::Id` (no IdKey) nor a plain struct (must re-root).
            "node_tree" => Ok(Field::Embedded(EmbeddedSlot::NodeTree, StructRef::Borrowed(&self.node_tree))),
            "diffuse_color" => Ok(Field::Value(Value::FloatArray {
                data: small_f32(&self.diffuse_color),
                shape: ArrayShape::Fixed(4),
                tag: math_tag(prop),
            })),
            "metallic" => Ok(Field::Value(Value::Float(self.metallic))),
            "roughness" => Ok(Field::Value(Value::Float(self.roughness))),
            _ => id_get(self.srna(), &self.header, prop),
        }
    }
}

impl RnaMut for Material {
    fn set(&mut self, prop: &'static PropDef, v: Value) -> Result<SetOutcome, RnaError> {
        match prop.identifier {
            "diffuse_color" => {
                self.diffuse_color = f32_array::<4>(&v)?;
                Ok(SetOutcome::Done)
            }
            "metallic" => match v {
                Value::Float(f) => {
                    self.metallic = f;
                    Ok(SetOutcome::Done)
                }
                _ => Err(bad_value("metallic")),
            },
            "roughness" => match v {
                Value::Float(f) => {
                    self.roughness = f;
                    Ok(SetOutcome::Done)
                }
                _ => Err(bad_value("roughness")),
            },
            _ => {
                let srna = self.srna();
                id_set(srna, &mut self.header, prop, v)
            }
        }
    }
    fn struct_mut(&mut self, prop: &'static PropDef) -> Result<RnaMutGuard<'_>, RnaError> {
        match prop.identifier {
            // The mutable walk into an embedded ID goes through the owner (owner gen bump, §5.1).
            "node_tree" => Ok(Box::new(&mut self.node_tree)),
            id => Err(unbound(self.srna(), id)),
        }
    }
    fn coll_mut(&mut self, prop: &'static PropDef) -> Result<RnaCollMutGuard<'_>, RnaError> {
        Err(unbound(self.srna(), prop.identifier))
    }
}
