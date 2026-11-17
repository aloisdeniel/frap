// SPDX-License-Identifier: GPL-3.0-or-later
//! §5.2 `Main` (= `bpy.data`), `IdMap`, `ChangeLog`.

use std::collections::BTreeMap;
use std::sync::Arc;

use frap_probe_mesh::Mesh;
use frap_probe_rna::*;
use indexmap::{IndexMap, IndexSet};
use smol_str::SmolStr;

use crate::{Material, Object, Scene};

#[derive(Default, Debug, Clone, PartialEq)]
pub struct ChangeLog {
    pub added: Vec<IdAny>,
    pub removed: Vec<IdAny>,
    pub modified: IndexSet<IdAny>,
    pub renamed: Vec<(IdAny, SmolStr)>,
    pub context_changed: bool,
}

#[derive(Clone)]
pub struct IdMap<T: IdType> {
    items: IndexMap<IdKey, Arc<T>>,
    by_name: BTreeMap<(SmolStr, Option<IdKey>), IdKey>,
}

impl<T: IdType> Default for IdMap<T> {
    fn default() -> Self {
        IdMap {
            items: IndexMap::new(),
            by_name: BTreeMap::new(),
        }
    }
}

impl<T: IdType> IdMap<T> {
    pub fn get(&self, r: IdRef<T::Kind>) -> Option<&T> {
        self.items.get(&r.key()).map(|a| &**a)
    }
    /// RFC 0001 F-MAIN-2: `HostRead::mesh_view` returns `Arc<Mesh>`; without this accessor the
    /// object-mode path must deep-clone the mesh.
    pub fn get_arc(&self, r: IdRef<T::Kind>) -> Option<Arc<T>> {
        self.items.get(&r.key()).cloned()
    }
    /// `Arc::make_mut` + header.gen bump + log.modified (§5.2).
    pub fn get_mut(&mut self, r: IdRef<T::Kind>, log: &mut ChangeLog) -> Option<&mut T> {
        let a = self.items.get_mut(&r.key())?;
        let t = Arc::make_mut(a);
        t.header_mut().generation = next_gen();
        log.modified.insert(r.any());
        Some(t)
    }
    pub fn find(&self, name: &str) -> Option<IdRef<T::Kind>> {
        self.by_name.get(&(SmolStr::from(name), None)).map(|k| IdRef::from_key(*k))
    }
    pub fn index_of(&self, r: IdRef<T::Kind>) -> Option<usize> {
        self.items.get_index_of(&r.key())
    }
    pub fn at(&self, i: isize) -> Option<IdRef<T::Kind>> {
        let n = self.items.len() as isize;
        let i = if i < 0 { n + i } else { i };
        if i < 0 {
            return None;
        }
        self.items.get_index(i as usize).map(|(k, _)| IdRef::from_key(*k))
    }
    /// Probe: no unique_name / sorted ListBase order (§5.4); insertion order.
    pub fn insert(&mut self, requested: &str, mut value: T, log: &mut ChangeLog) -> IdRef<T::Kind> {
        let key = IdKey::next();
        value.header_mut().name = requested.into();
        value.header_mut().generation = next_gen();
        self.items.insert(key, Arc::new(value));
        self.by_name.insert((requested.into(), None), key);
        let r = IdRef::from_key(key);
        log.added.push(r.any());
        r
    }
    pub fn iter(&self) -> impl Iterator<Item = (IdRef<T::Kind>, &T)> {
        self.items.iter().map(|(k, v)| (IdRef::from_key(*k), &**v))
    }
    pub fn len(&self) -> usize {
        self.items.len()
    }
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

#[derive(Clone, Default)]
pub struct Main {
    pub objects: IdMap<Object>,
    pub meshes: IdMap<Mesh>,
    pub materials: IdMap<Material>,
    pub scenes: IdMap<Scene>,
    pub is_dirty: bool,
}

impl Main {
    /// RFC 0001 F-MAIN-1: dynamic access by `IdAny` (resolver re-rooting, hooks, `IdNames`).
    pub fn id(&self, a: IdAny) -> Option<&dyn Rna> {
        match a.code {
            IdCode::OB => self.objects.get(IdRef::from_any(a)?).map(|x| x as &dyn Rna),
            IdCode::ME => self.meshes.get(IdRef::from_any(a)?).map(|x| x as &dyn Rna),
            IdCode::MA => self.materials.get(IdRef::from_any(a)?).map(|x| x as &dyn Rna),
            IdCode::SC => self.scenes.get(IdRef::from_any(a)?).map(|x| x as &dyn Rna),
            _ => None,
        }
    }
    pub fn id_mut(&mut self, a: IdAny, log: &mut ChangeLog) -> Option<&mut dyn RnaMut> {
        match a.code {
            IdCode::OB => self.objects.get_mut(IdRef::from_any(a)?, log).map(|x| x as &mut dyn RnaMut),
            IdCode::ME => self.meshes.get_mut(IdRef::from_any(a)?, log).map(|x| x as &mut dyn RnaMut),
            IdCode::MA => self.materials.get_mut(IdRef::from_any(a)?, log).map(|x| x as &mut dyn RnaMut),
            IdCode::SC => self.scenes.get_mut(IdRef::from_any(a)?, log).map(|x| x as &mut dyn RnaMut),
            _ => None,
        }
    }
    pub fn header(&self, a: IdAny) -> Option<&IdHeader> {
        match a.code {
            IdCode::OB => self.objects.get(IdRef::from_any(a)?).map(IdType::header),
            IdCode::ME => self.meshes.get(IdRef::from_any(a)?).map(IdType::header),
            IdCode::MA => self.materials.get(IdRef::from_any(a)?).map(IdType::header),
            IdCode::SC => self.scenes.get(IdRef::from_any(a)?).map(IdType::header),
            _ => None,
        }
    }
    /// `bpy.data.<collection>` by attribute name: (code, ordered ids, item StructDef).
    pub fn collection(&self, name: &str) -> Option<(Vec<IdAny>, &'static StructDef)> {
        fn ids<T: IdType>(m: &IdMap<T>) -> Vec<IdAny> {
            m.iter().map(|(r, _)| r.any()).collect()
        }
        Some(match name {
            "objects" => (ids(&self.objects), &frap_probe_spec::OBJECT),
            "meshes" => (ids(&self.meshes), &frap_probe_spec::MESH),
            "materials" => (ids(&self.materials), &frap_probe_spec::MATERIAL),
            "scenes" => (ids(&self.scenes), &frap_probe_spec::SCENE),
            _ => return None,
        })
    }
}

impl IdNames for Main {
    fn id_name(&self, id: IdAny) -> Option<(SmolStr, Option<SmolStr>)> {
        self.header(id).map(|h| (h.name.clone(), None))
    }
}
