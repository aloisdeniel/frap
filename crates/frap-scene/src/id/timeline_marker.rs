// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.types.TimelineMarker` (`Scene.timeline_markers`, `TimelineMarkers.new/remove/clear`).
//!
//! M7 W0 (decision log M7W0-8) froze the data; WP 7.6 owns this file: the RNA binding
//! (`name`, `frame`, `select`, `camera`; the set hooks are `hooks/timeline_marker_*.rs`), the
//! collection proxy of `Scene.timeline_markers` (`hooks/scene_timeline_markers.rs`), and marker
//! camera switching (`BKE_scene_camera_switch_find` / `_update`). The `TimelineMarkers`
//! functions live in `frap-ops-data` (`funcs/timeline_markers_*.rs`), the `marker.*` operators
//! in `frap-ops-anim`.
//!
//! Ported from blender/source/blender/makesrna/intern/{rna_timeline,rna_scene}.cc and
//! blender/source/blender/blenkernel/intern/scene.cc (Blender 5.2.4, GPL-2.0-or-later).
//! Verified on 5.2.4: marker names are not unique (`timeline_markers.keys()` lists `['B', 'B']`);
//! `timeline_markers['B']` is the first one; a marker's repr has no path
//! (`bpy.data.scenes['Scene']...TimelineMarker`), so frap addresses markers by index
//! (`timeline_markers[1]`, as material slots are); `new` selects the marker and appends it.

use frap_rna::{
    IdRef, Item, ObjectId, PyExc, RnaColl, RnaError, RnaErrorKind, RnaPtr, Root, SceneId, Seg,
    StructDef, StructRef,
};
use smol_str::SmolStr;

use crate::data::{ChangeLog, Main};
use crate::id::Scene;

/// A timeline marker (DNA `TimeMarker`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TimelineMarker {
    /// `name`
    pub name: SmolStr,
    /// `frame`
    pub frame: i32,
    /// `select`
    pub select: bool,
    /// `camera`: the camera the scene switches to from this marker on.
    pub camera: Option<IdRef<ObjectId>>,
}

// The four properties are fields; their set hooks (`hooks/timeline_marker_*.rs`) find the
// marker by index and apply the update callbacks (`rna_TimelineMarker_camera_update`).
frap_rna::rna_struct! { TimelineMarker as frap_spec_rna::TIMELINE_MARKER {
    name => name (set = hook),
    frame => frame (set = hook),
    select => select (set = hook),
    camera => hook,
}}

/// Runs `f` on the marker `this` names, mutably; [`removed`] when it is gone.
pub fn with_marker_mut<R>(
    main: &mut Main,
    log: &mut ChangeLog,
    this: &RnaPtr,
    f: impl FnOnce(&mut TimelineMarker) -> R,
) -> Result<R, RnaError> {
    let (scene, i) = marker_of(main, this)?.ok_or_else(removed)?;
    let sc = main.scenes.get_mut(scene, log).ok_or_else(removed)?;
    let m = sc.timeline_markers.get_mut(i).ok_or_else(removed)?;
    Ok(f(m))
}

/// `sizeof(TimeMarker::name) - 1`: names are cut to 63 bytes (`STRNCPY_UTF8`).
pub const NAME_MAX_BYTES: usize = 63;

/// `MAXFRAME`
pub const MAXFRAME: i32 = 1_048_574;

impl TimelineMarker {
    /// `rna_TimeLine_add` / `ed_marker_add_exec`: a selected marker named `name` (cut to 63
    /// bytes, on a character boundary) at `frame`, without camera.
    pub fn new(name: &str, frame: i32) -> TimelineMarker {
        TimelineMarker {
            name: SmolStr::new(frap_rna::naming::truncate_utf8(name, NAME_MAX_BYTES)),
            frame,
            select: true,
            camera: None,
        }
    }

    /// The name operators give a marker at `frame` (`SNPRINTF_UTF8(marker->name, "F_%02d",
    /// frame)`): `F_05`, `F_120`, `F_-3`.
    pub fn frame_name(frame: i32) -> SmolStr {
        SmolStr::new(format!("F_{frame:02}"))
    }
}

/// `Scene.timeline_markers` as an RNA collection: items keyed by name (the first match) but
/// addressed by index, since names repeat.
pub struct TimelineMarkerColl<'a> {
    /// The scene's markers.
    pub markers: &'a [TimelineMarker],
}

impl RnaColl for TimelineMarkerColl<'_> {
    fn item_type(&self) -> &'static StructDef {
        &frap_spec_rna::TIMELINE_MARKER
    }
    fn len(&self) -> usize {
        self.markers.len()
    }
    fn key_of(&self, i: usize) -> Option<SmolStr> {
        self.markers.get(i).map(|m| m.name.clone())
    }
    fn find(&self, key: &str) -> Option<usize> {
        self.markers.iter().position(|m| m.name == key)
    }
    fn item(&self, i: usize) -> Result<Item<'_>, RnaError> {
        self.markers
            .get(i)
            .map(|m| Item::Struct(StructRef::Borrowed(m)))
            .ok_or_else(|| frap_rna::traits::index_out_of_range(i, self.markers.len()))
    }
    fn item_seg(&self, i: usize) -> Seg {
        Seg::Index(i as i64)
    }
}

/// An internal error (a hook got a pointer it cannot serve).
fn internal(msg: &str) -> RnaError {
    RnaError::new(RnaErrorKind::Other, PyExc::RuntimeError, msg.to_owned())
}

/// A marker that is gone (Blender raises `ReferenceError: StructRNA of type TimelineMarker has
/// been removed` for a Python object of a removed marker; frap has no variables, so only a stale
/// pointer inside one statement gets here, as `RuntimeError`).
pub fn removed() -> RnaError {
    RnaError::new(
        RnaErrorKind::Other,
        PyExc::RuntimeError,
        "StructRNA of type TimelineMarker has been removed",
    )
}

/// The scene a `TimelineMarker` / `TimelineMarkers` pointer is rooted at.
pub fn scene_of(this: &RnaPtr) -> Result<IdRef<SceneId>, RnaError> {
    match this.root {
        Root::Id(a) => a.typed(),
        _ => None,
    }
    .ok_or_else(|| internal("frap internal: the pointer is not rooted at a Scene"))
}

/// The scene and marker index a `TimelineMarker` pointer names
/// (`timeline_markers[<index or name>]`); `None` when the marker is gone.
pub fn marker_of(main: &Main, this: &RnaPtr) -> Result<Option<(IdRef<SceneId>, usize)>, RnaError> {
    let scene = scene_of(this)?;
    let Some(sc) = main.scenes.get(scene) else {
        return Ok(None);
    };
    let i = match this.path.as_slice() {
        [Seg::Prop("timeline_markers"), Seg::Index(i)] => {
            let n = sc.timeline_markers.len() as i64;
            let i = if *i < 0 { i + n } else { *i };
            (0..n).contains(&i).then_some(i as usize)
        }
        [Seg::Prop("timeline_markers"), Seg::Key(k)] => {
            sc.timeline_markers.iter().position(|m| m.name == *k)
        }
        _ => return Err(internal("frap internal: not a timeline marker pointer")),
    };
    Ok(i.map(|i| (scene, i)))
}

/// The pointer to marker `i` of `scene`.
pub fn marker_ptr(scene: IdRef<SceneId>, i: usize) -> RnaPtr {
    RnaPtr::id(scene.any(), &frap_spec_rna::SCENE)
        .child(
            Seg::Prop("timeline_markers"),
            &frap_spec_rna::TIMELINE_MARKER,
        )
        .child(Seg::Index(i as i64), &frap_spec_rna::TIMELINE_MARKER)
}

// Ported from blender/source/blender/blenkernel/intern/scene.cc (`BKE_scene_camera_switch_find`,
// Blender 5.2.4, GPL-2.0-or-later): the camera of the last marker at or before `time` (a marker
// exactly at `time` wins at once), else of the left-most marker; markers whose camera has
// `hide_render` are skipped (`OB_HIDE_RENDER`). Frames outside ±(MAXFRAME + 1) keep Blender's
// integer comparisons.
/// The camera the markers of `scene` select at frame `time`, if any.
pub fn camera_switch_find(main: &Main, scene: &Scene, time: i32) -> Option<IdRef<ObjectId>> {
    let mut frame = -(MAXFRAME + 1);
    let mut min_frame = MAXFRAME + 1;
    let mut camera = None;
    let mut first_camera = None;
    for m in &scene.timeline_markers {
        let Some(c) = m.camera else { continue };
        if main.objects.get(c).is_none_or(|o| o.hide_render) {
            continue;
        }
        if m.frame <= time && m.frame > frame {
            camera = Some(c);
            frame = m.frame;
            if frame == time {
                break;
            }
        }
        if m.frame < min_frame {
            first_camera = Some(c);
            min_frame = m.frame;
        }
    }
    camera.or(first_camera)
}

/// `BKE_scene_camera_switch_update`: makes the marker camera of the scene's current frame the
/// scene camera. Returns whether `scene.camera` changed.
pub fn camera_switch_update(main: &mut Main, log: &mut ChangeLog, scene: IdRef<SceneId>) -> bool {
    camera_switch_update_at(main, log, scene, None)
}

/// [`camera_switch_update`] at `frame` instead of the scene's `frame_current` (a render frame:
/// `RE_RenderAnim` sets `scene->r.cfra` before the switch).
pub fn camera_switch_update_at(
    main: &mut Main,
    log: &mut ChangeLog,
    scene: IdRef<SceneId>,
    frame: Option<i32>,
) -> bool {
    let Some(sc) = main.scenes.get(scene) else {
        return false;
    };
    let time = frame.unwrap_or(sc.frame_current);
    match camera_switch_find(main, sc, time) {
        Some(c) if sc.camera != Some(c) => {
            if let Some(sc) = main.scenes.get_mut(scene, log) {
                sc.camera = Some(c);
            }
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `BKE_scene_camera_switch_find` (cases verified on 5.2.4): the last marker on or before
    /// the frame wins, a marker exactly on it at once; before every marker the left-most one;
    /// `hide_render` cameras and markers without camera are skipped.
    #[test]
    fn switch_finds_the_markers_camera() {
        let (mut main, win) = crate::startup::factory();
        let mut log = ChangeLog::default();
        let cam = main.objects.find("Camera").unwrap();
        let cube = main.objects.find("Cube").unwrap();
        {
            let s = main.scenes.get_mut(win.scene, &mut log).unwrap();
            s.timeline_markers = vec![
                TimelineMarker {
                    camera: Some(cube),
                    ..TimelineMarker::new("A", 10)
                },
                TimelineMarker::new("Plain", 2),
                TimelineMarker {
                    camera: Some(cam),
                    ..TimelineMarker::new("B", 5)
                },
                TimelineMarker {
                    camera: Some(cam),
                    ..TimelineMarker::new("Far", MAXFRAME + 5)
                },
            ];
        }
        let find =
            |main: &Main, t| camera_switch_find(main, main.scenes.get(win.scene).unwrap(), t);
        assert_eq!(find(&main, 7), Some(cam));
        assert_eq!(find(&main, 10), Some(cube));
        assert_eq!(find(&main, 100), Some(cube));
        assert_eq!(find(&main, -50), Some(cam), "left-most marker");
        main.objects.get_mut(cam, &mut log).unwrap().hide_render = true;
        assert_eq!(
            find(&main, 7),
            Some(cube),
            "hide_render cameras are skipped"
        );
        assert_eq!(find(&main, -50), Some(cube));
        // The update switches only when the camera changes.
        {
            let s = main.scenes.get_mut(win.scene, &mut log).unwrap();
            s.frame_current = 12;
            s.camera = Some(cam);
        }
        assert!(camera_switch_update(&mut main, &mut log, win.scene));
        assert_eq!(main.scenes.get(win.scene).unwrap().camera, Some(cube));
        assert!(!camera_switch_update(&mut main, &mut log, win.scene));
        assert!(!camera_switch_update_at(
            &mut main,
            &mut log,
            win.scene,
            Some(-3)
        ));
    }

    #[test]
    fn bindings_match_the_rna_definition() {
        frap_rna::testing::check_bindings::<TimelineMarker>();
    }

    /// Markers are addressed by index; names repeat, and a keyed pointer is the first match.
    #[test]
    fn markers_are_found_by_index_or_first_name() {
        let (mut main, win) = crate::startup::factory();
        let mut log = ChangeLog::default();
        main.scenes
            .get_mut(win.scene, &mut log)
            .unwrap()
            .timeline_markers = vec![TimelineMarker::new("B", 1), TimelineMarker::new("B", 2)];
        let p = marker_ptr(win.scene, 1);
        assert_eq!(marker_of(&main, &p).unwrap(), Some((win.scene, 1)));
        let keyed = RnaPtr::id(win.scene.any(), &frap_spec_rna::SCENE)
            .child(
                Seg::Prop("timeline_markers"),
                &frap_spec_rna::TIMELINE_MARKER,
            )
            .child(Seg::Key("B".into()), &frap_spec_rna::TIMELINE_MARKER);
        assert_eq!(marker_of(&main, &keyed).unwrap(), Some((win.scene, 0)));
        assert_eq!(marker_of(&main, &marker_ptr(win.scene, 5)).unwrap(), None);
        let long = TimelineMarker::new(&"é".repeat(40), 0);
        assert_eq!(long.name.len(), 62, "cut on a character boundary");
    }
}
