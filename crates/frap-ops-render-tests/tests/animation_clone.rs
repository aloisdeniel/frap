// SPDX-License-Identifier: GPL-3.0-or-later
//! Deviation D-07 (WP 6.4, ARCHITECTURE §7.3): `render.render` renders a clone of the state
//! with the animation of the frame applied (`Host::snapshot_at`) and never writes the session's
//! originals. Blender's render depsgraph flushes the animation to the originals, overwriting a
//! manual edit of an animated channel; frap keeps the edit, while the pixels still show the
//! animated value. Needs a GPU adapter (a visible skip under `FRAP_GPU_OPTIONAL=1`).

mod common;

use common::Env;

/// Workbench, small, the factory camera.
const SETUP: &str = "bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.render.resolution_x = 64
bpy.context.scene.render.resolution_y = 36
bpy.data.objects['Cube'].keyframe_insert('location', frame=1)
bpy.data.objects['Cube'].location.x = 2.0
bpy.data.objects['Cube'].keyframe_insert('location', frame=11)
bpy.context.scene.frame_set(11)";

fn location_x(e: &mut Env) -> f64 {
    let j = e.run("bpy.data.objects['Cube'].location.x");
    j["value"].as_f64().expect("a float")
}

#[test]
fn render_clone_keeps_manual_edits_of_animated_channels() {
    let mut e = Env::new("anim_clone");
    e.ok(SETUP);
    assert_eq!(location_x(&mut e), 2.0, "frame_set applied the animation");

    // A manual edit of the animated channel, far out of the camera's view.
    e.ok("bpy.data.objects['Cube'].location.x = 400.0");
    e.ok("bpy.context.scene.render.filepath = 'edited.png'");
    let Some(r) = e.render("bpy.ops.render.render(write_still=True)") else {
        return;
    };
    assert_eq!(r["status"], "FINISHED", "{r}");
    // frap: the original keeps the edit (Blender would read 2.0 here: D-07).
    assert_eq!(location_x(&mut e), 400.0);
    // The same frame without the manual edit renders the same pixels: the clone applied the
    // animated value (x = 2), not the edit.
    e.ok("bpy.data.objects['Cube'].location.x = 2.0");
    e.ok("bpy.context.scene.render.filepath = 'animated.png'");
    e.render("bpy.ops.render.render(write_still=True)")
        .expect("GPU available above");
    let edited = std::fs::read(e.path("edited.png")).expect("edited.png written");
    let animated = std::fs::read(e.path("animated.png")).expect("animated.png written");
    assert_eq!(edited, animated, "the clone renders the animated value");
    // Without animation the edit is what renders (the cube leaves the view).
    e.ok("bpy.data.objects['Cube'].animation_data_clear()");
    e.ok("bpy.data.objects['Cube'].location.x = 400.0");
    e.ok("bpy.context.scene.render.filepath = 'static.png'");
    e.render("bpy.ops.render.render(write_still=True)")
        .expect("GPU available above");
    let stat = std::fs::read(e.path("static.png")).expect("static.png written");
    assert_ne!(stat, animated, "the edit moves the cube out of view");
    assert_eq!(location_x(&mut e), 400.0);
}
