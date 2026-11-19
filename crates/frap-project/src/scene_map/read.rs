// SPDX-License-Identifier: GPL-3.0-or-later
//! `scene.json` → `Main` + window state (WP 1.23). Two passes: every ID is first created
//! empty with its name and library (so references can point forward and `IdKey`s are fresh),
//! then filled.

use std::collections::HashMap;
use std::sync::Arc;

use frap_anim::{Action, AnimData};
use frap_mesh::{AttrData, Attribute, AttributeSet, DeformVert, Domain, Mesh};
use frap_nodes::generated::node_type;
use frap_nodes::{
    InOut, InterfaceSocket, Link, Node, NodeTree, NodeTreeType, Socket, SocketType, SocketValue,
};
use frap_rna::{IdAny, IdCode, IdHeader, IdKind, IdRef, IdType, LibraryId, Value};
use frap_scene::generated::modifier_settings::{ModifierSettings, ModifierType};
use frap_scene::id::{
    Armature, Camera, Collection, EncodedImage, Image, Key, Library, Light, Material, Modifier,
    Object, PointCloud, Pose, Scene, SlotLink, SlotOverride, Texture, World,
};
use frap_scene::{
    Base, BaseFlag, ChangeLog, IdMap, LayerCollection, LayerCollectionPath, Main, ObjectMode,
    Preferences, ViewLayer, VirtualView3D, WindowState,
};
use serde_json::{Map, Value as J};
use smol_str::SmolStr;

use super::json::{self, Obj, array, de, farr, idprop_from, int, num, num64, string};
use super::{LINKED, MapError};
use crate::buffers::{BufferStore, Element, ImageRef, ImageStore};

/// `(code, name, library name)` → the ID created for it.
type Table = HashMap<(IdCode, String, Option<String>), IdAny>;

/// What the reader needs while filling IDs.
pub(crate) struct Cx<'a> {
    pub(crate) strict: bool,
    table: Table,
    buffers: &'a BufferStore,
    images: &'a ImageStore,
    /// Messages for the operator's reports (WARNING).
    pub(crate) warnings: Vec<String>,
}

/// One entry of a collection: name, library name, JSON.
struct Entry<'a> {
    name: &'a str,
    lib: Option<&'a str>,
    v: &'a J,
}

fn entries<'a>(root: &'a Map<String, J>, key: &str) -> Result<Vec<Entry<'a>>, MapError> {
    let Some(v) = root.get(key) else {
        return Ok(Vec::new());
    };
    let J::Object(m) = v else {
        return Err(MapError::invalid(key, "expected an object"));
    };
    let mut out = Vec::new();
    for (k, v) in m {
        if k == LINKED {
            let J::Object(libs) = v else {
                return Err(MapError::invalid(
                    &format!("{key}.{LINKED}"),
                    "expected an object",
                ));
            };
            for (lib, ids) in libs {
                let J::Object(ids) = ids else {
                    return Err(MapError::invalid(
                        &format!("{key}.{LINKED}.{lib}"),
                        "expected an object",
                    ));
                };
                for (name, v) in ids {
                    out.push(Entry {
                        name,
                        lib: Some(lib),
                        v,
                    });
                }
            }
        } else {
            out.push(Entry {
                name: k,
                lib: None,
                v,
            });
        }
    }
    Ok(out)
}

fn epath(coll: &str, e: &Entry) -> String {
    match e.lib {
        None => format!("{coll}[{:?}]", e.name),
        Some(l) => format!("{coll}[{:?}, {l:?}]", e.name),
    }
}

impl Cx<'_> {
    /// Resolves `{"$id", "name", "library"}` (or `null`).
    pub(crate) fn idref(&self, v: &J, path: &str) -> Result<Option<IdAny>, MapError> {
        if v.is_null() {
            return Ok(None);
        }
        let mut o = Obj::new(v, path, self.strict)?;
        let code = string(o.req("$id")?, path)?;
        let code = IdCode::from_code(code)
            .ok_or_else(|| MapError::invalid(path, format!("unknown ID code {code:?}")))?;
        let name = string(o.req("name")?, path)?.to_owned();
        let lib = match o.opt("library") {
            None | Some(J::Null) => None,
            Some(l) => Some(string(l, path)?.to_owned()),
        };
        o.finish()?;
        self.table
            .get(&(code, name.clone(), lib.clone()))
            .copied()
            .map(Some)
            .ok_or_else(|| {
                MapError::invalid(
                    path,
                    format!(
                        "reference to a missing ID {}[{name:?}{}]",
                        code.collection(),
                        lib.map(|l| format!(", {l:?}")).unwrap_or_default()
                    ),
                )
            })
    }
    fn typed<K: IdKind>(&self, v: &J, path: &str) -> Result<Option<IdRef<K>>, MapError> {
        match self.idref(v, path)? {
            None => Ok(None),
            Some(a) => IdRef::from_any(a).map(Some).ok_or_else(|| {
                MapError::invalid(path, format!("expected a reference to {}", K::COLLECTION))
            }),
        }
    }
    fn opt_typed<K: IdKind>(
        &self,
        o: &mut Obj,
        key: &str,
        out: &mut Option<IdRef<K>>,
    ) -> Result<(), MapError> {
        if let Some(v) = o.opt(key) {
            *out = self.typed(v, &o.sub(key))?;
        }
        Ok(())
    }
    fn decode<T: Element>(&self, v: &J, path: &str) -> Result<Vec<T>, MapError> {
        self.buffers
            .decode(v)
            .map_err(|e| MapError::invalid(path, e.to_string()))
    }
    fn anim(&self, o: &mut Obj, out: &mut Option<AnimData>) -> Result<(), MapError> {
        let Some(v) = o.opt("animation_data") else {
            return Ok(());
        };
        if v.is_null() {
            *out = None;
            return Ok(());
        }
        let mut a = Obj::new(v, o.sub("animation_data"), self.strict)?;
        let mut ad = AnimData::default();
        self.opt_typed(&mut a, "action", &mut ad.action)?;
        a.i32("action_slot_handle", &mut ad.action_slot_handle)?;
        a.smol("last_slot_identifier", &mut ad.last_slot_identifier)?;
        a.bool("use_nla", &mut ad.use_nla)?;
        a.f32("action_influence", &mut ad.action_influence)?;
        a.finish()?;
        *out = Some(ad);
        Ok(())
    }
}

/// Reads the ID header keys every datablock has.
fn header(h: &mut IdHeader, o: &mut Obj) -> Result<(), MapError> {
    o.bool("use_fake_user", &mut h.use_fake_user)?;
    o.bool("is_linked_packed", &mut h.is_linked_packed)?;
    if let Some(v) = o.opt("id_properties") {
        let p = o.sub("id_properties");
        let J::Object(m) = v else {
            return Err(MapError::invalid(&p, "expected an object"));
        };
        h.custom = m
            .iter()
            .map(|(k, x)| Ok((SmolStr::new(k), idprop_from(x, &format!("{p}[{k:?}]"))?)))
            .collect::<Result<_, MapError>>()?;
    }
    Ok(())
}

/// The result of reading a `scene.json`.
pub(crate) struct Read {
    pub(crate) main: Main,
    pub(crate) win: WindowState,
    pub(crate) prefs: Preferences,
    pub(crate) warnings: Vec<String>,
}

/// Creates the empty ID of every entry of `key` (pass 1).
fn create<T: IdType>(
    map: &mut IdMap<T>,
    root: &Map<String, J>,
    key: &str,
    libs: &HashMap<String, IdRef<LibraryId>>,
    table: &mut Table,
    log: &mut ChangeLog,
    new: impl Fn(&str) -> T,
) -> Result<(), MapError> {
    for e in entries(root, key)? {
        let mut t = new(e.name);
        if let Some(l) = e.lib {
            let lib = libs.get(l).ok_or_else(|| {
                MapError::invalid(&epath(key, &e), format!("unknown library {l:?}"))
            })?;
            t.header_mut().library = Some(*lib);
        }
        let r = map.insert(e.name, t, log);
        if map.name_of(r) != Some(e.name) {
            return Err(MapError::invalid(
                &epath(key, &e),
                "duplicate or invalid name",
            ));
        }
        table.insert(
            (T::Kind::CODE, e.name.to_owned(), e.lib.map(str::to_owned)),
            r.any(),
        );
    }
    Ok(())
}

/// Fills every ID of `key` (pass 2).
fn fill<T: IdType>(
    map: &mut IdMap<T>,
    root: &Map<String, J>,
    key: &str,
    cx: &mut Cx,
    log: &mut ChangeLog,
    mut f: impl FnMut(&mut T, &mut Obj, &mut Cx) -> Result<(), MapError>,
) -> Result<(), MapError> {
    for e in entries(root, key)? {
        let a = cx.table[&(T::Kind::CODE, e.name.to_owned(), e.lib.map(str::to_owned))];
        let r: IdRef<T::Kind> = IdRef::from_any(a).expect("invariant: table holds this kind");
        let t = map.get_mut(r, log).expect("invariant: created in pass 1");
        let mut o = Obj::new(e.v, epath(key, &e), cx.strict)?;
        header(t.header_mut(), &mut o)?;
        f(t, &mut o, cx)?;
        o.finish()?;
    }
    Ok(())
}

/// Reads a whole `scene.json` tree.
pub(crate) fn read(
    root: &J,
    buffers: &BufferStore,
    images: &ImageStore,
    strict: bool,
) -> Result<Read, MapError> {
    let J::Object(root) = root else {
        return Err(MapError::invalid("scene.json", "expected an object"));
    };
    const KEYS: [&str; 18] = [
        "window",
        "libraries",
        "scenes",
        "collections",
        "objects",
        "meshes",
        "materials",
        "node_groups",
        "cameras",
        "lights",
        "worlds",
        "images",
        "actions",
        "armatures",
        "shape_keys",
        "textures",
        "pointclouds",
        "preferences",
    ];
    if strict && let Some(k) = root.keys().find(|k| !KEYS.contains(&k.as_str())) {
        return Err(MapError::Unknown(k.clone()));
    }
    let mut main = Main::default();
    let mut log = ChangeLog::default();
    let mut table = Table::new();

    // Pass 1: libraries, then every other ID, empty.
    let mut libs = HashMap::new();
    for e in entries(root, "libraries")? {
        if e.lib.is_some() {
            return Err(MapError::invalid(
                &epath("libraries", &e),
                "a library cannot be linked",
            ));
        }
        let r = main
            .libraries
            .insert(e.name, Library::new(e.name, ""), &mut log);
        if main.libraries.name_of(r) != Some(e.name) {
            return Err(MapError::invalid(
                &epath("libraries", &e),
                "duplicate or invalid name",
            ));
        }
        libs.insert(e.name.to_owned(), r);
        table.insert((IdCode::LI, e.name.to_owned(), None), r.any());
    }
    let t = &mut table;
    let l = &mut log;
    create(&mut main.scenes, root, "scenes", &libs, t, l, |n| {
        Scene::new(n, Vec::new())
    })?;
    create(
        &mut main.collections,
        root,
        "collections",
        &libs,
        t,
        l,
        |n| Collection::new(n),
    )?;
    create(&mut main.objects, root, "objects", &libs, t, l, |n| {
        Object::new(n, None)
    })?;
    create(&mut main.meshes, root, "meshes", &libs, t, l, |n| {
        Mesh::new(n)
    })?;
    create(
        &mut main.materials,
        root,
        "materials",
        &libs,
        t,
        l,
        Material::new,
    )?;
    create(
        &mut main.node_groups,
        root,
        "node_groups",
        &libs,
        t,
        l,
        |n| NodeTree::new(n, NodeTreeType::Shader, false),
    )?;
    create(&mut main.cameras, root, "cameras", &libs, t, l, Camera::new)?;
    create(&mut main.lights, root, "lights", &libs, t, l, |n| {
        Light::new(n, "POINT")
    })?;
    create(&mut main.worlds, root, "worlds", &libs, t, l, World::new)?;
    create(&mut main.images, root, "images", &libs, t, l, |n| {
        Image::new(n, 0, 0)
    })?;
    create(&mut main.actions, root, "actions", &libs, t, l, |n| {
        Action::new(n)
    })?;
    create(&mut main.armatures, root, "armatures", &libs, t, l, |n| {
        Armature::new(n)
    })?;
    create(&mut main.shape_keys, root, "shape_keys", &libs, t, l, |n| {
        Key::new(n)
    })?;
    create(&mut main.textures, root, "textures", &libs, t, l, |n| {
        Texture::new(n)
    })?;
    create(
        &mut main.pointclouds,
        root,
        "pointclouds",
        &libs,
        t,
        l,
        |n| PointCloud::new(n),
    )?;

    let mut cx = Cx {
        strict,
        table,
        buffers,
        images,
        warnings: Vec::new(),
    };
    let cx = &mut cx;
    fill(
        &mut main.libraries,
        root,
        "libraries",
        cx,
        l,
        |lib, o, cx| {
            o.smol("filepath", &mut lib.filepath)?;
            cx.opt_typed(o, "parent", &mut lib.parent)?;
            o.bool("is_archive", &mut lib.is_archive)?;
            Ok(())
        },
    )?;
    fill(&mut main.scenes, root, "scenes", cx, l, scene)?;
    fill(
        &mut main.collections,
        root,
        "collections",
        cx,
        l,
        collection,
    )?;
    fill(&mut main.objects, root, "objects", cx, l, object)?;
    fill(&mut main.meshes, root, "meshes", cx, l, mesh)?;
    fill(&mut main.materials, root, "materials", cx, l, |m, o, cx| {
        let mut t = o.obj("node_tree")?;
        tree(&mut m.node_tree, &mut t, cx)?;
        t.finish()?;
        o.smol("surface_render_method", &mut m.surface_render_method)?;
        o.smol("blend_method", &mut m.blend_method)?;
        o.bool("use_backface_culling", &mut m.use_backface_culling)?;
        o.arr("diffuse_color", &mut m.diffuse_color)?;
        o.f32("metallic", &mut m.metallic)?;
        o.f32("roughness", &mut m.roughness)?;
        o.f32("alpha_threshold", &mut m.alpha_threshold)?;
        o.i32("pass_index", &mut m.pass_index)?;
        o.bool("use_nodes", &mut m.use_nodes)?;
        cx.anim(o, &mut m.animation_data)
    })?;
    fill(&mut main.node_groups, root, "node_groups", cx, l, tree)?;
    fill(&mut main.cameras, root, "cameras", cx, l, camera)?;
    fill(&mut main.lights, root, "lights", cx, l, |li, o, cx| {
        o.smol("type", &mut li.r#type)?;
        cx.anim(o, &mut li.animation_data)?;
        let rest = rest(o);
        li.fields = de(&J::Object(rest), &o.path, cx.strict)?;
        Ok(())
    })?;
    fill(&mut main.worlds, root, "worlds", cx, l, |w, o, cx| {
        let mut t = o.obj("node_tree")?;
        tree(&mut w.node_tree, &mut t, cx)?;
        t.finish()?;
        o.arr("color", &mut w.color)?;
        o.bool("use_nodes", &mut w.use_nodes)?;
        cx.anim(o, &mut w.animation_data)
    })?;
    fill(&mut main.images, root, "images", cx, l, image)?;
    // Header-only datablocks (their fields arrive with M3/M6).
    fill(&mut main.actions, root, "actions", cx, l, |_, _, _| Ok(()))?;
    fill(&mut main.armatures, root, "armatures", cx, l, |_, _, _| {
        Ok(())
    })?;
    fill(
        &mut main.shape_keys,
        root,
        "shape_keys",
        cx,
        l,
        |_, _, _| Ok(()),
    )?;
    fill(&mut main.textures, root, "textures", cx, l, |t, o, cx| {
        // Files written before M4 W0 have no texture fields: keep the fresh values.
        if o.opt("type").is_none() {
            return Ok(());
        }
        o.smol("type", &mut t.r#type)?;
        cx.anim(o, &mut t.animation_data)?;
        let image = match o.opt("image") {
            Some(v) => cx.idref(v, &o.sub("image"))?,
            None => None,
        };
        let rest = rest(o);
        t.fields = de(&J::Object(rest), &o.path, cx.strict)?;
        t.fields.image = image;
        Ok(())
    })?;
    fill(
        &mut main.pointclouds,
        root,
        "pointclouds",
        cx,
        l,
        pointcloud,
    )?;

    let win = window(root.get("window"), &main, cx)?;
    let prefs = prefs(root.get("preferences"), cx.strict)?;
    main.is_dirty = false;
    Ok(Read {
        main,
        win,
        prefs,
        warnings: std::mem::take(&mut cx.warnings),
    })
}

/// Every key not read yet, as a map (for the generated structs).
fn rest(o: &mut Obj) -> Map<String, J> {
    o.rest()
}

fn window(v: Option<&J>, main: &Main, cx: &Cx) -> Result<WindowState, MapError> {
    let v = v.ok_or_else(|| MapError::invalid("window", "missing"))?;
    let mut o = Obj::new(v, "window", cx.strict)?;
    let scene_name = string(o.req("scene")?, "window.scene")?;
    let scene = main
        .scenes
        .find_in(scene_name, None)
        .ok_or_else(|| MapError::invalid("window.scene", format!("no scene {scene_name:?}")))?;
    let mut view_layer = SmolStr::default();
    o.smol("view_layer", &mut view_layer)?;
    let (_, factory_win) = frap_scene::startup::empty();
    let mut view3d = VirtualView3D {
        view_matrix: factory_win.view3d.view_matrix,
        shading: factory_win.view3d.shading,
    };
    if let Some(v3) = o.opt("view3d") {
        let mut v = Obj::new(v3, "window.view3d", cx.strict)?;
        v.mat4("view_matrix", &mut view3d.view_matrix)?;
        if let Some(s) = v.opt("shading") {
            view3d.shading = de(s, "window.view3d.shading", cx.strict)?;
        }
        v.finish()?;
    }
    o.finish()?;
    Ok(WindowState {
        scene,
        view_layer,
        view3d,
    })
}

fn prefs(v: Option<&J>, strict: bool) -> Result<Preferences, MapError> {
    let mut p = Preferences::default();
    let Some(v) = v else {
        return Ok(p);
    };
    let mut o = Obj::new(v, "preferences", strict)?;
    if let Some(addons) = o.opt("addons") {
        let J::Object(addons) = addons else {
            return Err(MapError::invalid(
                "preferences.addons",
                "expected an object",
            ));
        };
        for (module, values) in addons {
            let path = format!("preferences.addons[{module:?}]");
            let J::Object(values) = values else {
                return Err(MapError::invalid(&path, "expected an object"));
            };
            let mut m = indexmap::IndexMap::new();
            for (k, x) in values {
                let val = match x {
                    J::Bool(b) => Value::Bool(*b),
                    J::Number(n) if n.is_f64() => Value::Float(num(x, &path)?),
                    J::Number(_) => Value::Int(x.as_i64().unwrap_or(0)),
                    J::String(s) => Value::Str(SmolStr::new(s)),
                    _ => {
                        return Err(MapError::invalid(
                            &format!("{path}.{k}"),
                            "unsupported value",
                        ));
                    }
                };
                m.insert(SmolStr::new(k), val);
            }
            p.addons.insert(SmolStr::new(module), m);
        }
    }
    o.finish()?;
    Ok(p)
}

fn scene(s: &mut Scene, o: &mut Obj, cx: &mut Cx) -> Result<(), MapError> {
    cx.opt_typed(o, "camera", &mut s.camera)?;
    cx.opt_typed(o, "world", &mut s.world)?;
    o.i32("frame_start", &mut s.frame_start)?;
    o.i32("frame_end", &mut s.frame_end)?;
    o.i32("frame_current", &mut s.frame_current)?;
    o.i32("frame_step", &mut s.frame_step)?;
    macro_rules! plain {
        ($($f:ident),*) => {$(
            if let Some(v) = o.opt(stringify!($f)) {
                s.$f = de(v, &o.sub(stringify!($f)), cx.strict)?;
            }
        )*};
    }
    plain!(
        render,
        view_settings,
        display_settings,
        display,
        eevee,
        cursor,
        tool_settings,
        unit_settings
    );
    let mut c = o.obj("collection")?;
    c.smol("name", &mut s.collection.header.name)?;
    collection(&mut s.collection, &mut c, cx)?;
    c.finish()?;
    let p = o.sub("view_layers");
    s.view_layers = array(o.req("view_layers")?, &p)?
        .iter()
        .enumerate()
        .map(|(i, v)| view_layer(v, &format!("{p}[{i}]"), cx))
        .collect::<Result<_, _>>()?;
    cx.anim(o, &mut s.animation_data)
}

fn view_layer(v: &J, path: &str, cx: &Cx) -> Result<ViewLayer, MapError> {
    let mut o = Obj::new(v, path, cx.strict)?;
    let mut vl = ViewLayer {
        name: SmolStr::default(),
        bases: Vec::new(),
        active: None,
        layer_collection: LayerCollection::new(None),
        active_layer_collection: LayerCollectionPath::default(),
    };
    o.smol("name", &mut vl.name)?;
    let p = o.sub("bases");
    for (i, b) in array(o.req("bases")?, &p)?.iter().enumerate() {
        let bp = format!("{p}[{i}]");
        let mut bo = Obj::new(b, &bp, cx.strict)?;
        let object = cx
            .typed(bo.req("object")?, &bp)?
            .ok_or_else(|| MapError::invalid(&bp, "a base needs an object"))?;
        let (mut select, mut hide) = (false, false);
        bo.bool("select", &mut select)?;
        bo.bool("hide", &mut hide)?;
        bo.finish()?;
        let mut flag = BaseFlag::empty();
        flag.set(BaseFlag::SELECTED, select);
        flag.set(BaseFlag::HIDDEN, hide);
        vl.bases.push(Base { object, flag });
    }
    cx.opt_typed(&mut o, "active", &mut vl.active)?;
    let lp = o.sub("layer_collection");
    vl.layer_collection = layer_collection(o.req("layer_collection")?, &lp, cx)?;
    let ap = o.sub("active_layer_collection");
    vl.active_layer_collection = LayerCollectionPath(
        array(o.req("active_layer_collection")?, &ap)?
            .iter()
            .map(|x| {
                x.as_u64()
                    .and_then(|i| u16::try_from(i).ok())
                    .ok_or_else(|| MapError::invalid(&ap, "expected child indices"))
            })
            .collect::<Result<_, _>>()?,
    );
    o.finish()?;
    Ok(vl)
}

fn layer_collection(v: &J, path: &str, cx: &Cx) -> Result<LayerCollection, MapError> {
    let mut o = Obj::new(v, path, cx.strict)?;
    let mut lc = LayerCollection::new(None);
    cx.opt_typed(&mut o, "collection", &mut lc.collection)?;
    o.bool("exclude", &mut lc.exclude)?;
    o.bool("hide_viewport", &mut lc.hide_viewport)?;
    o.bool("holdout", &mut lc.holdout)?;
    o.bool("indirect_only", &mut lc.indirect_only)?;
    let p = o.sub("children");
    if let Some(c) = o.opt("children") {
        lc.children = array(c, &p)?
            .iter()
            .enumerate()
            .map(|(i, c)| layer_collection(c, &format!("{p}[{i}]"), cx))
            .collect::<Result<_, _>>()?;
    }
    o.finish()?;
    Ok(lc)
}

fn collection(c: &mut Collection, o: &mut Obj, cx: &mut Cx) -> Result<(), MapError> {
    if let Some(v) = o.opt("objects") {
        let p = o.sub("objects");
        c.objects = array(v, &p)?
            .iter()
            .map(|x| {
                cx.typed(x, &p)?
                    .ok_or_else(|| MapError::invalid(&p, "null object"))
            })
            .collect::<Result<_, _>>()?;
    }
    if let Some(v) = o.opt("children") {
        let p = o.sub("children");
        c.children = array(v, &p)?
            .iter()
            .map(|x| {
                cx.typed(x, &p)?
                    .ok_or_else(|| MapError::invalid(&p, "null collection"))
            })
            .collect::<Result<_, _>>()?;
    }
    o.bool("hide_select", &mut c.hide_select)?;
    o.bool("hide_viewport", &mut c.hide_viewport)?;
    o.bool("hide_render", &mut c.hide_render)?;
    o.smol("color_tag", &mut c.color_tag)?;
    Ok(())
}

fn object(ob: &mut Object, o: &mut Obj, cx: &mut Cx) -> Result<(), MapError> {
    // `type` is derived from `data`; read for strictness only.
    let _ = o.opt("type");
    if let Some(v) = o.opt("data") {
        ob.data = cx.idref(v, &o.sub("data"))?;
    }
    cx.opt_typed(o, "parent", &mut ob.parent)?;
    o.smol("parent_type", &mut ob.parent_type)?;
    o.smol("parent_bone", &mut ob.parent_bone)?;
    o.iarr("parent_vertices", &mut ob.parent_vertices)?;
    o.mat4("matrix_parent_inverse", &mut ob.matrix_parent_inverse)?;
    o.arr("location", &mut ob.location)?;
    o.smol("rotation_mode", &mut ob.rotation_mode)?;
    o.arr("rotation_euler", &mut ob.rotation_euler)?;
    o.arr("rotation_quaternion", &mut ob.rotation_quaternion)?;
    o.arr("rotation_axis_angle", &mut ob.rotation_axis_angle)?;
    o.arr("scale", &mut ob.scale)?;
    o.arr("delta_location", &mut ob.delta_location)?;
    o.arr("delta_rotation_euler", &mut ob.delta_rotation_euler)?;
    o.arr(
        "delta_rotation_quaternion",
        &mut ob.delta_rotation_quaternion,
    )?;
    o.arr("delta_scale", &mut ob.delta_scale)?;
    if let Some(v) = o.opt("mode") {
        let p = o.sub("mode");
        ob.mode = match string(v, &p)? {
            "OBJECT" => ObjectMode::Object,
            "POSE" => ObjectMode::Pose,
            // The operator re-enters Edit Mode after loading (the mesh → BMesh conversion is
            // `frap_op::helpers::edit_mode::reenter_after_load`, WP 2.2).
            "EDIT" => ObjectMode::Edit,
            other => return Err(MapError::invalid(&p, format!("unknown mode {other:?}"))),
        };
    }
    if let Some(v) = o.opt("modifiers") {
        let p = o.sub("modifiers");
        ob.modifiers = array(v, &p)?
            .iter()
            .enumerate()
            .map(|(i, m)| modifier(m, &format!("{p}[{i}]"), cx))
            .collect::<Result<_, _>>()?;
    }
    if let Some(v) = o.opt("material_slots") {
        let p = o.sub("material_slots");
        ob.material_slot_overrides = array(v, &p)?
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let sp = format!("{p}[{i}]");
                let mut so = Obj::new(s, &sp, cx.strict)?;
                let mut slot = SlotOverride::default();
                if let Some(l) = so.opt("link") {
                    slot.link = match string(l, &sp)? {
                        "DATA" => SlotLink::Data,
                        "OBJECT" => SlotLink::Object,
                        other => {
                            return Err(MapError::invalid(&sp, format!("unknown link {other:?}")));
                        }
                    };
                }
                cx.opt_typed(&mut so, "material", &mut slot.material)?;
                so.finish()?;
                Ok(slot)
            })
            .collect::<Result<_, MapError>>()?;
    }
    o.i32("active_material_index", &mut ob.active_material_index)?;
    o.i32(
        "vertex_groups_active_index",
        &mut ob.vertex_groups_active_index,
    )?;
    if let Some(v) = o.opt("pose") {
        ob.pose = (!v.is_null()).then(Pose::default);
    }
    o.bool("hide_viewport", &mut ob.hide_viewport)?;
    o.bool("hide_render", &mut ob.hide_render)?;
    o.bool("hide_select", &mut ob.hide_select)?;
    o.smol("display_type", &mut ob.display_type)?;
    o.arr("color", &mut ob.color)?;
    o.smol("empty_display_type", &mut ob.empty_display_type)?;
    o.f32("empty_display_size", &mut ob.empty_display_size)?;
    o.i32("active_shape_key_index", &mut ob.active_shape_key_index)?;
    o.bool("show_name", &mut ob.show_name)?;
    o.bool("show_axis", &mut ob.show_axis)?;
    o.bool("show_wire", &mut ob.show_wire)?;
    o.bool("show_all_edges", &mut ob.show_all_edges)?;
    o.bool("show_texture_space", &mut ob.show_texture_space)?;
    o.bool("show_in_front", &mut ob.show_in_front)?;
    o.bool("show_bounds", &mut ob.show_bounds)?;
    o.bool("show_only_shape_key", &mut ob.show_only_shape_key)?;
    o.smol("display_bounds_type", &mut ob.display_bounds_type)?;
    cx.anim(o, &mut ob.animation_data)
}

const MODIFIER_BASE: [&str; 10] = [
    "name",
    "type",
    "show_viewport",
    "show_render",
    "show_in_editmode",
    "show_on_cage",
    "show_expanded",
    "is_active",
    "use_pin_to_last",
    "persistent_uid",
];

fn modifier(v: &J, path: &str, cx: &Cx) -> Result<Modifier, MapError> {
    let strict = cx.strict;
    let mut o = Obj::new(v, path, strict)?;
    let ty = string(o.req("type")?, path)?;
    let mt = ModifierType::from_identifier(ty)
        .ok_or_else(|| MapError::invalid(path, format!("unknown modifier type {ty:?}")))?;
    let mut md = Modifier {
        name: SmolStr::default(),
        show_viewport: true,
        show_render: true,
        show_in_editmode: false,
        show_on_cage: false,
        show_expanded: true,
        is_active: false,
        use_pin_to_last: false,
        persistent_uid: 0,
        settings: ModifierSettings::new(mt),
    };
    o.smol("name", &mut md.name)?;
    o.bool("show_viewport", &mut md.show_viewport)?;
    o.bool("show_render", &mut md.show_render)?;
    o.bool("show_in_editmode", &mut md.show_in_editmode)?;
    o.bool("show_on_cage", &mut md.show_on_cage)?;
    o.bool("show_expanded", &mut md.show_expanded)?;
    o.bool("is_active", &mut md.is_active)?;
    o.bool("use_pin_to_last", &mut md.use_pin_to_last)?;
    o.u32("persistent_uid", &mut md.persistent_uid)?;
    // The settings' ID pointers (M4 W0; files written before have none).
    let mut ids = Vec::new();
    for (k, _) in md.settings.id_refs() {
        if let Some(v) = o.opt(k) {
            ids.push((k, cx.idref(v, &o.sub(k))?));
        }
    }
    let rest = o.rest();
    // Re-tag the flattened settings ({"Bevel": {…}}) on top of the type's fresh values.
    let mut tagged = json::ser(&md.settings)?;
    if let J::Object(t) = &mut tagged
        && let Some((_, J::Object(fields))) = t.iter_mut().next()
    {
        for (k, x) in rest {
            if MODIFIER_BASE.contains(&k.as_str()) {
                continue;
            }
            if strict && !fields.contains_key(&k) {
                return Err(MapError::Unknown(format!("{path}.{k}")));
            }
            fields.insert(k, x);
        }
    }
    md.settings = de(&tagged, path, strict)?;
    for (k, r) in ids {
        md.settings.set_id_ref(k, r);
    }
    o.finish()?;
    Ok(md)
}

fn domain(s: &str, path: &str) -> Result<Domain, MapError> {
    Ok(match s {
        "POINT" => Domain::Point,
        "EDGE" => Domain::Edge,
        "FACE" => Domain::Face,
        "CORNER" => Domain::Corner,
        other => return Err(MapError::invalid(path, format!("unknown domain {other:?}"))),
    })
}

/// The `attributes` array of a mesh or point cloud.
fn attributes(o: &mut Obj, cx: &mut Cx) -> Result<AttributeSet, MapError> {
    let mut attrs = AttributeSet::default();
    let ap = o.sub("attributes");
    for (i, a) in array(o.req("attributes")?, &ap)?.iter().enumerate() {
        let p = format!("{ap}[{i}]");
        let mut ao = Obj::new(a, &p, cx.strict)?;
        let name = SmolStr::new(string(ao.req("name")?, &p)?);
        let dom = domain(string(ao.req("domain")?, &p)?, &p)?;
        let ty = string(ao.req("data_type")?, &p)?;
        let d = ao.req("data")?;
        let data = match ty {
            "FLOAT" => AttrData::Float(Arc::new(cx.decode(d, &p)?)),
            "INT" => AttrData::Int(Arc::new(cx.decode(d, &p)?)),
            "INT8" => AttrData::Int8(Arc::new(cx.decode(d, &p)?)),
            "BOOLEAN" => AttrData::Bool(Arc::new(cx.decode(d, &p)?)),
            "FLOAT2" => AttrData::Float2(Arc::new(cx.decode(d, &p)?)),
            "FLOAT_VECTOR" => AttrData::Float3(Arc::new(cx.decode(d, &p)?)),
            "FLOAT4" => AttrData::Float4(Arc::new(cx.decode(d, &p)?)),
            "FLOAT_COLOR" => AttrData::FloatColor(Arc::new(cx.decode(d, &p)?)),
            "QUATERNION" => AttrData::Quaternion(Arc::new(cx.decode(d, &p)?)),
            "BYTE_COLOR" => AttrData::ByteColor(Arc::new(cx.decode(d, &p)?)),
            "INT16_2D" => AttrData::Int16x2(Arc::new(cx.decode(d, &p)?)),
            "INT32_2D" => AttrData::Int32x2(Arc::new(cx.decode(d, &p)?)),
            "FLOAT4X4" => {
                let flat: Vec<[f32; 16]> = cx.decode(d, &p)?;
                AttrData::Float4x4(Arc::new(
                    flat.iter()
                        .map(|f| std::array::from_fn(|r| std::array::from_fn(|c| f[r * 4 + c])))
                        .collect(),
                ))
            }
            "STRING" => AttrData::String(Arc::new(
                array(d, &p)?
                    .iter()
                    .map(|s| Ok(SmolStr::new(string(s, &p)?)))
                    .collect::<Result<_, MapError>>()?,
            )),
            other => {
                return Err(MapError::invalid(
                    &p,
                    format!("unknown data_type {other:?}"),
                ));
            }
        };
        ao.finish()?;
        attrs.insert(Attribute {
            name,
            domain: dom,
            data,
        });
    }
    Ok(attrs)
}

/// A point cloud (WP 3.7): `points`, `attributes` (POINT domain, one value per point),
/// `materials`, `animation_data`. Files written before WP 3.7 have none of them: kept empty.
fn pointcloud(pc: &mut PointCloud, o: &mut Obj, cx: &mut Cx) -> Result<(), MapError> {
    let Some(n) = o.opt("points") else {
        return Ok(());
    };
    let p = o.sub("points");
    let n = n
        .as_u64()
        .ok_or_else(|| MapError::invalid(&p, "expected a point count"))? as usize;
    let mut out = PointCloud::new(pc.header.name.clone());
    out.resize(n);
    for a in attributes(o, cx)?.iter() {
        let ap = format!("{}[{:?}]", o.sub("attributes"), a.name);
        if a.domain != Domain::Point {
            return Err(MapError::invalid(
                &ap,
                "point cloud attributes are POINT attributes",
            ));
        }
        out.set_attribute(&a.name, a.data.clone())
            .map_err(|e| MapError::invalid(&ap, e))?;
    }
    if let Some(v) = o.opt("materials") {
        let p = o.sub("materials");
        out.materials = array(v, &p)?
            .iter()
            .map(|x| cx.typed(x, &p))
            .collect::<Result<_, _>>()?;
    }
    cx.anim(o, &mut out.animation_data)?;
    out.header = pc.header.clone();
    *pc = out;
    Ok(())
}

fn mesh(me: &mut Mesh, o: &mut Obj, cx: &mut Cx) -> Result<(), MapError> {
    let mut m = Mesh::new(me.header.name.clone());
    let fp = o.sub("face_offsets");
    let offsets: Vec<i32> = cx.decode(o.req("face_offsets")?, &fp)?;
    if offsets.first() != Some(&0) || offsets.windows(2).any(|w| w[1] < w[0]) {
        return Err(MapError::invalid(
            &fp,
            "face offsets must start at 0 and never decrease",
        ));
    }
    // Sets the offsets; the placeholder corner attributes are replaced below.
    let corners = *offsets.last().unwrap_or(&0) as usize;
    m.set_topology(offsets, vec![0; corners], Vec::new());
    let attrs = attributes(o, cx)?;
    *m.attributes_mut() = attrs;
    if let Some(dv) = o.opt("deform_verts")
        && !dv.is_null()
    {
        let p = o.sub("deform_verts");
        let mut d = Obj::new(dv, &p, cx.strict)?;
        let offsets: Vec<i32> = cx.decode(d.req("offsets")?, &p)?;
        let groups: Vec<u32> = cx.decode(d.req("groups")?, &p)?;
        let weights: Vec<f32> = cx.decode(d.req("weights")?, &p)?;
        d.finish()?;
        if offsets.len() != m.verts_num() + 1 || groups.len() != weights.len() {
            return Err(MapError::invalid(&p, "inconsistent deform-vertex arrays"));
        }
        let out = m.deform_verts_mut();
        for (v, w) in out.iter_mut().zip(offsets.windows(2)) {
            let (s, e) = (w[0] as usize, w[1] as usize);
            if s > e || e > groups.len() {
                return Err(MapError::invalid(&p, "bad deform-vertex offsets"));
            }
            *v = DeformVert {
                weights: groups[s..e]
                    .iter()
                    .copied()
                    .zip(weights[s..e].iter().copied())
                    .collect(),
            };
        }
    }
    if let Some(v) = o.opt("materials") {
        let p = o.sub("materials");
        m.materials = array(v, &p)?
            .iter()
            .map(|x| cx.typed(x, &p))
            .collect::<Result<_, _>>()?;
    }
    if let Some(v) = o.opt("vertex_group_names") {
        let p = o.sub("vertex_group_names");
        m.vertex_group_names = array(v, &p)?
            .iter()
            .map(|x| Ok(SmolStr::new(string(x, &p)?)))
            .collect::<Result<_, MapError>>()?;
    }
    cx.opt_typed(o, "shape_keys", &mut m.shape_keys)?;
    o.opt_smol("uv_active", &mut m.uv_active)?;
    o.opt_smol("uv_active_render", &mut m.uv_active_render)?;
    o.opt_smol("color_active", &mut m.color_active)?;
    o.opt_smol("color_default", &mut m.color_default)?;
    o.opt_smol("attributes_active", &mut m.attributes_active)?;
    o.bool("use_auto_texspace", &mut m.texspace.auto)?;
    o.arr("texspace_location", &mut m.texspace.location)?;
    o.arr("texspace_size", &mut m.texspace.size)?;
    m.header = me.header.clone();
    *me = m;
    Ok(())
}

pub(crate) fn socket_type(s: &str, path: &str) -> Result<SocketType, MapError> {
    Ok(match s {
        "VALUE" => SocketType::Value,
        "INT" => SocketType::Int,
        "BOOLEAN" => SocketType::Boolean,
        "VECTOR" => SocketType::Vector,
        "RGBA" => SocketType::Rgba,
        "SHADER" => SocketType::Shader,
        "STRING" => SocketType::String,
        "ROTATION" => SocketType::Rotation,
        "MATRIX" => SocketType::Matrix,
        "MENU" => SocketType::Menu,
        "GEOMETRY" => SocketType::Geometry,
        "OBJECT" => SocketType::Object,
        "IMAGE" => SocketType::Image,
        "COLLECTION" => SocketType::Collection,
        "MATERIAL" => SocketType::Material,
        "TEXTURE" => SocketType::Texture,
        "CUSTOM" => SocketType::Custom,
        other => {
            return Err(MapError::invalid(
                path,
                format!("unknown socket type {other:?}"),
            ));
        }
    })
}

fn socket_value(t: SocketType, v: &J, path: &str, cx: &Cx) -> Result<SocketValue, MapError> {
    let ids = matches!(
        t,
        SocketType::Object
            | SocketType::Image
            | SocketType::Collection
            | SocketType::Material
            | SocketType::Texture
    );
    if ids {
        return Ok(SocketValue::Id(cx.idref(v, path)?));
    }
    if v.is_null() {
        return Ok(SocketValue::None);
    }
    Ok(match t {
        SocketType::Value => SocketValue::Float(num(v, path)?),
        SocketType::Int => SocketValue::Int(int(v, path)?),
        SocketType::Boolean => SocketValue::Bool(
            v.as_bool()
                .ok_or_else(|| MapError::invalid(path, "expected a boolean"))?,
        ),
        SocketType::Vector => SocketValue::Vector(farr(v, path)?),
        SocketType::Rgba => SocketValue::Color(farr(v, path)?),
        SocketType::Rotation => SocketValue::Rotation(farr(v, path)?),
        SocketType::String => SocketValue::String(SmolStr::new(string(v, path)?)),
        SocketType::Menu => SocketValue::Menu(SmolStr::new(string(v, path)?)),
        _ => {
            return Err(MapError::invalid(
                path,
                "this socket type has no default value",
            ));
        }
    })
}

fn socket(v: &J, path: &str, cx: &Cx) -> Result<Socket, MapError> {
    let mut o = Obj::new(v, path, cx.strict)?;
    let ty = socket_type(string(o.req("type")?, path)?, path)?;
    let mut s = Socket {
        name: SmolStr::default(),
        identifier: SmolStr::default(),
        ty,
        enabled: true,
        hide_value: false,
        default_value: SocketValue::None,
    };
    o.smol("identifier", &mut s.identifier)?;
    o.smol("name", &mut s.name)?;
    o.bool("enabled", &mut s.enabled)?;
    o.bool("hide_value", &mut s.hide_value)?;
    if let Some(d) = o.opt("default_value") {
        s.default_value = socket_value(ty, d, &o.sub("default_value"), cx)?;
    }
    o.finish()?;
    Ok(s)
}

/// A node property value, typed like the node type's default.
fn value_like(like: &Value, v: &J, path: &str, cx: &Cx) -> Result<Value, MapError> {
    let bad = || MapError::invalid(path, format!("value {v} does not fit {like:?}"));
    Ok(match like {
        Value::None if v.is_null() => Value::None,
        Value::Bool(_) => Value::Bool(v.as_bool().ok_or_else(bad)?),
        Value::Int(_) => Value::Int(v.as_i64().ok_or_else(bad)?),
        Value::Float(_) => Value::Float(num(v, path)?),
        Value::Double(_) => Value::Double(num64(v, path)?),
        Value::Str(_) => Value::Str(SmolStr::new(string(v, path)?)),
        Value::Enum(_) => Value::Enum(SmolStr::new(string(v, path)?)),
        Value::EnumSet(_) => Value::EnumSet(
            array(v, path)?
                .iter()
                .map(|x| Ok(SmolStr::new(string(x, path)?)))
                .collect::<Result<_, MapError>>()?,
        ),
        Value::BoolArray(_) => Value::BoolArray(
            array(v, path)?
                .iter()
                .map(|x| x.as_bool().ok_or_else(bad))
                .collect::<Result<_, _>>()?,
        ),
        Value::IntArray(_) => Value::IntArray(
            array(v, path)?
                .iter()
                .map(|x| x.as_i64().ok_or_else(bad))
                .collect::<Result<_, _>>()?,
        ),
        Value::FloatArray { shape, tag, .. } => Value::FloatArray {
            data: array(v, path)?
                .iter()
                .map(|x| num(x, path))
                .collect::<Result<_, _>>()?,
            shape: *shape,
            tag: *tag,
        },
        Value::Id(_) => Value::Id(cx.idref(v, path)?),
        _ => return Err(bad()),
    })
}

fn node(v: &J, path: &str, cx: &Cx) -> Result<Node, MapError> {
    let mut o = Obj::new(v, path, cx.strict)?;
    let bl = string(o.req("bl_idname")?, path)?;
    let Some(def) = node_type(bl) else {
        return builtin_group_node(bl, o, path, cx);
    };
    let mut uid = 0u32;
    o.u32("uid", &mut uid)?;
    let name = SmolStr::new(string(o.req("name")?, path)?);
    let mut n = Node::from_type(def, uid, name);
    o.string("label", &mut n.label)?;
    o.arr("location", &mut n.location)?;
    o.f32("width", &mut n.width)?;
    if let Some(p) = o.opt("parent") {
        n.parent = match p {
            J::Null => None,
            p => Some(
                p.as_u64()
                    .and_then(|x| u32::try_from(x).ok())
                    .ok_or_else(|| MapError::invalid(path, "bad parent uid"))?,
            ),
        };
    }
    o.bool("mute", &mut n.mute)?;
    o.bool("hide", &mut n.hide)?;
    for (key, out) in [("inputs", &mut n.inputs), ("outputs", &mut n.outputs)] {
        if let Some(v) = o.opt(key) {
            let p = o.sub(key);
            *out = array(v, &p)?
                .iter()
                .enumerate()
                .map(|(i, s)| socket(s, &format!("{p}[{i}]"), cx))
                .collect::<Result<_, _>>()?;
        }
    }
    if let Some(props) = o.opt("props") {
        let p = o.sub("props");
        let J::Object(props) = props else {
            return Err(MapError::invalid(&p, "expected an object"));
        };
        for (k, x) in props {
            let kp = format!("{p}.{k}");
            if let Some(slot) = n.props.values.get_mut(k.as_str()) {
                *slot = value_like(slot, x, &kp, cx)?;
                continue;
            }
            // A property of the node's RNA struct outside the generated defaults (the
            // startup file's `is_active_output`).
            match def.srna.find_prop(k) {
                Some(prop) => {
                    let like = Value::default_for(prop);
                    n.props
                        .values
                        .insert(prop.identifier, value_like(&like, x, &kp, cx)?);
                }
                None if cx.strict => return Err(MapError::Unknown(kp)),
                None => {}
            }
        }
    }
    o.finish()?;
    Ok(n)
}

/// A node of the built-in linked "Smooth by Angle" group whose geometry-node type is not in
/// the generated table (WP 4.1): rebuilt from its stored sockets and properties.
fn builtin_group_node(bl: &str, mut o: Obj, path: &str, cx: &Cx) -> Result<Node, MapError> {
    use frap_nodes::builtin_groups::smooth_by_angle::{NODE_PROPS, NODE_TYPES};
    let bl_idname = NODE_TYPES
        .iter()
        .copied()
        .find(|t| *t == bl)
        .ok_or_else(|| MapError::invalid(path, format!("unknown node type {bl:?}")))?;
    let mut n = Node {
        uid: 0,
        name: SmolStr::default(),
        label: String::new(),
        bl_idname,
        location: [0.0, 0.0],
        width: 140.0,
        parent: None,
        mute: false,
        hide: false,
        inputs: Vec::new(),
        outputs: Vec::new(),
        props: Default::default(),
    };
    o.u32("uid", &mut n.uid)?;
    n.name = SmolStr::new(string(o.req("name")?, path)?);
    o.string("label", &mut n.label)?;
    o.arr("location", &mut n.location)?;
    o.f32("width", &mut n.width)?;
    if let Some(p) = o.opt("parent") {
        n.parent = p.as_u64().and_then(|x| u32::try_from(x).ok());
    }
    o.bool("mute", &mut n.mute)?;
    o.bool("hide", &mut n.hide)?;
    for (key, out) in [("inputs", &mut n.inputs), ("outputs", &mut n.outputs)] {
        if let Some(v) = o.opt(key) {
            let p = o.sub(key);
            *out = array(v, &p)?
                .iter()
                .enumerate()
                .map(|(i, s)| socket(s, &format!("{p}[{i}]"), cx))
                .collect::<Result<_, _>>()?;
        }
    }
    if let Some(J::Object(props)) = o.opt("props") {
        for (k, x) in props {
            let Some(key) = NODE_PROPS.iter().copied().find(|p| *p == k.as_str()) else {
                continue;
            };
            let v = match x {
                J::String(s) => Value::Enum(SmolStr::new(s)),
                J::Bool(b) => Value::Bool(*b),
                _ => continue,
            };
            n.props.values.insert(key, v);
        }
    }
    o.finish()?;
    Ok(n)
}

fn tree(t: &mut NodeTree, o: &mut Obj, cx: &mut Cx) -> Result<(), MapError> {
    if let Some(n) = o.opt("name") {
        t.header.name = SmolStr::new(string(n, &o.sub("name"))?);
    }
    if let Some(b) = o.opt("bl_idname") {
        t.ty = match string(b, &o.sub("bl_idname"))? {
            "ShaderNodeTree" => NodeTreeType::Shader,
            "GeometryNodeTree" => NodeTreeType::Geometry,
            other => {
                return Err(MapError::invalid(
                    &o.sub("bl_idname"),
                    format!("unknown tree type {other:?}"),
                ));
            }
        };
    }
    let p = o.sub("nodes");
    t.nodes = array(o.req("nodes")?, &p)?
        .iter()
        .enumerate()
        .map(|(i, n)| node(n, &format!("{p}[{i}]"), cx))
        .collect::<Result<_, _>>()?;
    let p = o.sub("links");
    t.links = array(o.req("links")?, &p)?
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let lp = format!("{p}[{i}]");
            let mut lo = Obj::new(l, &lp, cx.strict)?;
            let mut link = Link {
                from_node: 0,
                from_socket: SmolStr::default(),
                to_node: 0,
                to_socket: SmolStr::default(),
            };
            lo.u32("from_node", &mut link.from_node)?;
            lo.smol("from_socket", &mut link.from_socket)?;
            lo.u32("to_node", &mut link.to_node)?;
            lo.smol("to_socket", &mut link.to_socket)?;
            lo.finish()?;
            Ok(link)
        })
        .collect::<Result<_, MapError>>()?;
    if let Some(v) = o.opt("interface") {
        let p = o.sub("interface");
        t.interface = array(v, &p)?
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let sp = format!("{p}[{i}]");
                let mut so = Obj::new(s, &sp, cx.strict)?;
                let socket_type = socket_type(string(so.req("socket_type")?, &sp)?, &sp)?;
                let in_out = match string(so.req("in_out")?, &sp)? {
                    "INPUT" => InOut::Input,
                    "OUTPUT" => InOut::Output,
                    other => return Err(MapError::invalid(&sp, format!("bad in_out {other:?}"))),
                };
                let mut s = InterfaceSocket {
                    name: SmolStr::default(),
                    identifier: SmolStr::default(),
                    in_out,
                    socket_type,
                    default_value: SocketValue::None,
                };
                so.smol("name", &mut s.name)?;
                so.smol("identifier", &mut s.identifier)?;
                if let Some(d) = so.opt("default_value") {
                    s.default_value = socket_value(socket_type, d, &sp, cx)?;
                }
                so.finish()?;
                Ok(s)
            })
            .collect::<Result<_, MapError>>()?;
    }
    o.u32("next_node_uid", &mut t.next_node_uid)?;
    o.u32("next_interface_uid", &mut t.next_interface_uid)?;
    cx.anim(o, &mut t.animation_data)
}

fn camera(c: &mut Camera, o: &mut Obj, cx: &mut Cx) -> Result<(), MapError> {
    cx.anim(o, &mut c.animation_data)?;
    let custom_shader = match o.opt("custom_shader") {
        Some(v) => cx.idref(v, &o.sub("custom_shader"))?,
        None => None,
    };
    let mut rest = rest(o);
    let mut focus = None;
    if let Some(J::Object(dof)) = rest.get_mut("dof")
        && let Some(f) = dof.remove("focus_object")
    {
        focus = cx.idref(&f, &format!("{}.dof.focus_object", o.path))?;
    }
    c.fields = de(&J::Object(rest), &o.path, cx.strict)?;
    c.fields.custom_shader = custom_shader;
    c.fields.dof.focus_object = focus;
    Ok(())
}

fn image(i: &mut Image, o: &mut Obj, cx: &mut Cx) -> Result<(), MapError> {
    o.smol("source", &mut i.source)?;
    o.string("filepath", &mut i.filepath)?;
    if let Some(p) = o.opt("packed_file") {
        let path = o.sub("packed_file");
        i.packed_file = match p {
            J::Null => None,
            p => {
                let mut po = Obj::new(p, &path, cx.strict)?;
                let r: ImageRef = serde_json::from_value(po.req("data")?.clone())
                    .map_err(|e| MapError::invalid(&path, e.to_string()))?;
                let mut mime = SmolStr::default();
                po.smol("mime", &mut mime)?;
                // `ImagePackedFile.filepath` (absent before M3: the image's own path).
                i.packed_filepath = i.filepath.clone();
                po.string("filepath", &mut i.packed_filepath)?;
                po.finish()?;
                let bytes = cx
                    .images
                    .get(&r)
                    .ok_or_else(|| MapError::invalid(&path, format!("missing {}", r.path())))?;
                Some(Arc::new(EncodedImage {
                    bytes: bytes.to_vec(),
                    mime,
                }))
            }
        };
    }
    o.smol("file_format", &mut i.file_format)?;
    o.smol("generated_type", &mut i.generated_type)?;
    o.i32("generated_width", &mut i.generated_width)?;
    o.i32("generated_height", &mut i.generated_height)?;
    o.arr("generated_color", &mut i.generated_color)?;
    o.bool("use_generated_float", &mut i.use_generated_float)?;
    let mut depth = i32::from(i.generated_depth);
    o.i32("generated_depth", &mut depth)?;
    i.generated_depth = u8::try_from(depth)
        .map_err(|_| MapError::invalid(&o.sub("generated_depth"), "expected 0..=255"))?;
    if let Some(v) = o.opt("colorspace_settings") {
        i.colorspace_settings = de(v, &o.sub("colorspace_settings"), cx.strict)?;
    }
    o.smol("alpha_mode", &mut i.alpha_mode)?;
    o.iarr("size", &mut i.size)?;
    cx.anim(o, &mut i.animation_data)
}
