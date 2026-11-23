# golden: startup=factory compare=objects,context tier=T0
# render-animation: files
# Render output write errors (M5+M6 stabilization): a filepath without a folder cannot be
# saved by the image writers (`BLI_file_ensure_parent_dir_exists` fails on the folder `''`:
# `Render error (No such file or directory) cannot save: 'st_0001.png'`), in an animation and
# for a still, while the OpenEXR writer opens it in the working directory; an OpenEXR whose
# folder cannot be made (a regular file stands in its way: unwritable on every OS) reports the
# open errno twice (`Error writing render result, …`); frame_step 0 renders the first frame
# only. `#@render` lines run in crates/frap-ops-render-tests (tests/render_animation.rs).
import bpy
bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.render.resolution_x = 16
bpy.context.scene.render.resolution_y = 8
bpy.context.scene.frame_end = 3
bpy.context.scene.render.filepath = 'st_'
#@render bpy.ops.render.render(animation=True)
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.frame_step = 0
#@render bpy.ops.render.render(animation=True)
bpy.context.scene.render.filepath = 'out/s0_'
#@render bpy.ops.render.render(animation=True)
bpy.context.scene.frame_step = 1
bpy.context.scene.render.image_settings.file_format = 'OPEN_EXR'
bpy.context.scene.render.filepath = 'out/s0_0001.png/y'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.cursor.location = (bpy.context.scene.frame_current, bpy.context.scene.frame_start, bpy.context.scene.frame_end)
