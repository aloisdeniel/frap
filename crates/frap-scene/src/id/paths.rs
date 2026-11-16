// SPDX-License-Identifier: GPL-3.0-or-later
//! Locating the struct a hook or RNA function is called on from its `this: &RnaPtr`
//! (§9.5). Struct-valued context members re-root at their canonical pointer (F-TR-2), so
//! scene-owned structs always arrive rooted at `Root::Id(scene)`:
//! `bpy.data.scenes['Scene'].view_layers["ViewLayer"].layer_collection.children["Collection"]`.

use frap_rna::{
    IdAny, IdCode, IdRef, ObjectId, PyExc, RnaError, RnaErrorKind, RnaPtr, Root, SceneId, Seg,
    Value,
};
use smallvec::SmallVec;

use crate::context::{LayerCollectionPath, ViewLayer};
use crate::data::Main;
use crate::id::Scene;

/// Internal error (the resolver handed a pointer this hook cannot serve).
pub(crate) fn internal(msg: impl Into<String>) -> RnaError {
    RnaError::new(RnaErrorKind::Other, PyExc::RuntimeError, msg.into())
}

/// The ID a pointer is rooted at.
pub(crate) fn root_id(this: &RnaPtr) -> Result<IdAny, RnaError> {
    match this.root {
        Root::Id(a) => Ok(a),
        Root::Embedded { owner, .. } => Ok(owner),
        _ => Err(internal(format!(
            "{} is not rooted at a datablock",
            this.srna.identifier
        ))),
    }
}

/// The object a pointer is rooted at.
pub(crate) fn object_of(this: &RnaPtr) -> Result<IdRef<ObjectId>, RnaError> {
    let a = root_id(this)?;
    IdRef::from_any(a).ok_or_else(|| internal("the pointer is not rooted at an Object"))
}

/// The scene a pointer is rooted at.
pub(crate) fn scene_of(this: &RnaPtr) -> Result<IdRef<SceneId>, RnaError> {
    let a = root_id(this)?;
    if a.code != IdCode::SC {
        return Err(internal("the pointer is not rooted at a Scene"));
    }
    IdRef::from_any(a).ok_or_else(|| internal("the pointer is not rooted at a Scene"))
}

/// Index of the item named by `seg` in a list of `names`.
fn pick(seg: &Seg, names: impl Iterator<Item = impl AsRef<str>>) -> Option<usize> {
    let names: Vec<_> = names.collect();
    match seg {
        Seg::Key(k) => names.iter().position(|n| n.as_ref() == k.as_str()),
        Seg::Index(i) => {
            let n = names.len() as i64;
            let i = if *i < 0 { i + n } else { *i };
            (0..n).contains(&i).then_some(i as usize)
        }
        Seg::Prop(_) => None,
    }
}

/// A view layer of a scene addressed by a pointer: `(scene, index, remaining segments)`.
pub(crate) fn view_layer_of<'p>(
    main: &Main,
    this: &'p RnaPtr,
) -> Result<(IdRef<SceneId>, usize, &'p [Seg]), RnaError> {
    let scene = scene_of(this)?;
    let sc = main
        .scenes
        .get(scene)
        .ok_or_else(|| internal("the scene was removed"))?;
    match this.path.as_slice() {
        [Seg::Prop("view_layers"), seg, rest @ ..] => {
            let i = pick(seg, sc.view_layers.iter().map(|v| v.name.as_str()))
                .ok_or_else(|| internal("view layer not found"))?;
            Ok((scene, i, rest))
        }
        _ => Err(internal(format!(
            "{} is not inside a view layer",
            this.srna.identifier
        ))),
    }
}

/// The view layer a pointer is inside.
pub(crate) fn view_layer<'m>(
    main: &'m Main,
    this: &RnaPtr,
) -> Result<(IdRef<SceneId>, usize, &'m ViewLayer), RnaError> {
    let (s, i, _) = view_layer_of(main, this)?;
    let vl = &main
        .scenes
        .get(s)
        .ok_or_else(|| internal("the scene was removed"))?
        .view_layers[i];
    Ok((s, i, vl))
}

/// A layer collection addressed by a pointer: scene, view layer index and child-index path.
pub(crate) fn layer_collection_of(
    main: &Main,
    this: &RnaPtr,
) -> Result<(IdRef<SceneId>, usize, LayerCollectionPath), RnaError> {
    let (scene, i, rest) = view_layer_of(main, this)?;
    let vl = &main
        .scenes
        .get(scene)
        .ok_or_else(|| internal("the scene was removed"))?
        .view_layers[i];
    let (mut path, mut rest): (SmallVec<[u16; 4]>, &[Seg]) = match rest {
        [Seg::Prop("layer_collection"), rest @ ..] => (SmallVec::new(), rest),
        [Seg::Prop("active_layer_collection"), rest @ ..] => {
            (vl.active_layer_collection.0.clone(), rest)
        }
        _ => return Err(internal("not a layer collection pointer")),
    };
    let mut lc = vl
        .layer_collection
        .at(&LayerCollectionPath(path.clone()))
        .ok_or_else(|| internal("layer collection not found"))?;
    loop {
        match rest {
            [] => break,
            [Seg::Prop("children"), seg, tail @ ..] => {
                let names: Vec<String> = lc
                    .children
                    .iter()
                    .map(|c| layer_collection_name(main, scene, c.collection))
                    .collect();
                let k = pick(seg, names.iter())
                    .ok_or_else(|| internal("layer collection not found"))?;
                path.push(k as u16);
                lc = &lc.children[k];
                rest = tail;
            }
            _ => return Err(internal("not a layer collection pointer")),
        }
    }
    Ok((scene, i, LayerCollectionPath(path)))
}

/// `LayerCollection.name`: the collection's name ("Scene Collection" for the master).
pub fn layer_collection_name(
    main: &Main,
    scene: IdRef<SceneId>,
    c: Option<IdRef<frap_rna::CollectionId>>,
) -> String {
    match c {
        Some(c) => main.collections.name_of(c).unwrap_or_default().to_owned(),
        None => main
            .scenes
            .get(scene)
            .map(|s| s.collection.header.name.to_string())
            .unwrap_or_default(),
    }
}

/// The canonical pointer of a view layer:
/// `bpy.data.scenes['Scene'].view_layers["ViewLayer"]`.
pub fn view_layer_ptr(scene: IdRef<SceneId>, name: &str) -> RnaPtr {
    RnaPtr::id(scene.any(), &frap_spec_rna::SCENE)
        .child(Seg::Prop("view_layers"), &frap_spec_rna::VIEW_LAYER)
        .child(Seg::Key(name.into()), &frap_spec_rna::VIEW_LAYER)
}

/// The canonical pointer of a layer collection (by collection name below the root).
pub fn layer_collection_ptr(
    main: &Main,
    scene: IdRef<SceneId>,
    vl: &ViewLayer,
    path: &LayerCollectionPath,
) -> RnaPtr {
    let mut p = view_layer_ptr(scene, &vl.name).child(
        Seg::Prop("layer_collection"),
        &frap_spec_rna::LAYER_COLLECTION,
    );
    let mut lc = &vl.layer_collection;
    for &i in &path.0 {
        let Some(c) = lc.children.get(i as usize) else {
            break;
        };
        p = p
            .child(Seg::Prop("children"), &frap_spec_rna::LAYER_COLLECTION)
            .child(
                Seg::Key(layer_collection_name(main, scene, c.collection).into()),
                &frap_spec_rna::LAYER_COLLECTION,
            );
        lc = c;
    }
    p
}

/// The view layer named by an optional `view_layer=` argument (a `Value::Ptr` to a view
/// layer), else the context view layer.
pub(crate) fn view_layer_arg(
    main: &Main,
    arg: Option<&Value>,
    default: (IdRef<SceneId>, &str),
) -> Result<(IdRef<SceneId>, usize), RnaError> {
    match arg {
        Some(Value::Ptr(p)) => {
            let (s, i, _) = view_layer_of(main, p)?;
            Ok((s, i))
        }
        _ => {
            let sc: &Scene = main
                .scenes
                .get(default.0)
                .ok_or_else(|| internal("the window scene was removed"))?;
            let i = sc
                .view_layers
                .iter()
                .position(|v| v.name == default.1)
                .unwrap_or(0);
            Ok((default.0, i))
        }
    }
}
