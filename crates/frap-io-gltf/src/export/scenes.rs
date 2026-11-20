// SPDX-License-Identifier: GPL-3.0-or-later
//! Scenes → glTF `scenes[]` (M3, WP 3.10): every Blender scene becomes a glTF scene, in
//! `bpy.data.scenes` order, with the user's (context) scene as the default `scene`; only the
//! context scene with `use_active_scene=True` or a `collection` export.
//!
//! Ported from io_scene_gltf2 5.2.41 `blender/exp/gather.py` (`gather_gltf2`,
//! `__gather_scene`, `__gather_name`) and `blender/exp/tree.py` (`VExportTree.construct`:
//! `bpy.context.window.scene = blender_scene`), Apache-2.0, The glTF-Blender-IO authors.
//! Each scene's tree is built in the context of that scene: its view layer (the window's view
//! layer name when the scene has it, else its first, `WM_window_set_active_scene`) decides the
//! selection, visibility and active-collection filters. The node gatherer runs once per
//! scene (each vnode has a fresh uuid, so an object in two scenes gives two glTF nodes), while
//! meshes, materials, textures and accessors are shared through the export-wide caches, as the
//! add-on's `@cached` gatherers share them (verified on 5.2.4). With `export_apply`, an object
//! with modifiers gets one glTF mesh per scene, from that scene's own depsgraph (the add-on's
//! key holds the temporary mesh of that depsgraph; M34S5-1).

use frap_gltf::json::{Index, Scene};
use frap_rna::{IdRef, SceneId};
use frap_scene::{Context, Main};

use crate::export::gather::Gather;
use crate::export::settings::ExportSettings;
use crate::export::{ExportError, extras, nodes, vtree};

/// `use_active_scene` is honoured here (M3, WP 3.10).
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::export::SUPPORTED_EXPORT_PROPS)]
static PROPS: &[&str] = &["use_active_scene"];

/// The scenes `gather_gltf2` exports, in order: every scene, or only the user's scene `user`
/// with `use_active_scene` or a collection export. The `export_apply` operator evaluates each
/// of them in its own depsgraph.
pub fn exported_scenes(
    main: &Main,
    user: IdRef<SceneId>,
    settings: &ExportSettings,
) -> Vec<IdRef<SceneId>> {
    if settings.collection.is_empty() && !settings.props.use_active_scene.value {
        main.scenes.iter().map(|(r, _)| r).collect()
    } else {
        vec![user]
    }
}

fn scenes_to_export(g: &Gather) -> Vec<IdRef<SceneId>> {
    exported_scenes(g.main, g.ctx.win.scene, g.settings)
}

/// Gathers the scene(s) and their node trees into `g.doc`, setting `g.doc.scene` to the
/// user's scene.
pub fn gather(g: &mut Gather) -> Result<(), ExportError> {
    let user = g.ctx.win.scene;
    let mut active: Option<u32> = None;
    for scene in scenes_to_export(g) {
        // `VExportTree.construct`: the window shows `blender_scene` while its tree is built.
        let mut win = g.ctx.win.clone();
        frap_scene::window::set_scene(g.main, &mut win, scene).map_err(ExportError::Invalid)?;
        let cx = Context {
            main: g.main,
            win: &win,
            prefs: g.ctx.prefs,
        };
        // `export_armature_object_remove` stays off once a scene's armatures refused it (WP 6.8).
        let settings = g.settings;
        let remove = g
            .state::<crate::export::skins::SkinState>()
            .armature_object_remove(settings);
        let tree = vtree::build_ex(g.main, &cx, g.settings, remove);
        if tree.armature_remove_refused {
            g.state::<crate::export::skins::SkinState>().remove_refused = true;
        }
        g.scene = scene;
        let mut roots: Vec<Index> = Vec::with_capacity(tree.roots.len());
        for &r in &tree.roots {
            roots.push(nodes::gather_node(g, &tree, r)?);
        }
        nodes::finish_scene(g, &tree)?;
        let Some(blender_scene) = g.main.scenes.get(scene) else {
            continue;
        };
        // `__gather_name`: the collection's name for a collection export.
        let name = if g.settings.collection.is_empty() {
            blender_scene.header.name.to_string()
        } else {
            g.settings.collection.clone()
        };
        let extras = extras::scene(g, blender_scene);
        g.doc.scenes.push(Scene {
            name: Some(name),
            nodes: roots,
            extras,
            ..Scene::default()
        });
        if scene == user {
            active = Some((g.doc.scenes.len() - 1) as u32);
        }
    }
    g.doc.scene = active;
    Ok(())
}
