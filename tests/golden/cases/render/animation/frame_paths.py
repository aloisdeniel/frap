# golden: startup=factory compare=objects,context tier=T0
# render-animation: files
# Frame paths of render.render(animation=True) (WP 6.6, RE_RenderAnim + BKE_image_path_from_imformat):
# `#` runs replaced by the zero-padded frame (the last run only), else 4 digits appended before
# the extension (also after a name that has one, and to a directory), negative frames, frames
# past 9999, use_file_extension off; the frames walk frame_start..frame_end by
# scene.frame_step, the operator's frame_start/frame_end replacing the scene's only when set.
# Every file written is compared with Blender's (names, empty or not). frame_current and the
# scene range are unchanged afterwards (cursor). `#@render` lines run in
# crates/frap-ops-render-tests (tests/render_animation.rs).
import bpy
bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.render.resolution_x = 16
bpy.context.scene.render.resolution_y = 8
bpy.context.scene.frame_current = 4
bpy.context.scene.frame_start = 2
bpy.context.scene.frame_end = 9
bpy.context.scene.frame_step = 3
bpy.context.scene.render.filepath = 'out/step_'
#@render bpy.ops.render.render(animation=True)
#@render bpy.ops.render.render(animation=True, frame_end=4)
#@render bpy.ops.render.render(animation=True, frame_start=7)
bpy.context.scene.frame_step = 1
bpy.context.scene.render.filepath = 'out/a_##_b_###'
#@render bpy.ops.render.render(animation=True, frame_start=9, frame_end=10)
bpy.context.scene.render.filepath = 'out/d_##/x'
#@render bpy.ops.render.render(animation=True, frame_start=1, frame_end=1)
bpy.context.scene.render.filepath = 'out/'
#@render bpy.ops.render.render(animation=True, frame_start=1, frame_end=1)
bpy.context.scene.render.filepath = 'out/x.png'
#@render bpy.ops.render.render(animation=True, frame_start=2, frame_end=2)
bpy.context.scene.render.filepath = 'out/y.jpg'
#@render bpy.ops.render.render(animation=True, frame_start=3, frame_end=3)
bpy.context.scene.render.filepath = 'out/big'
#@render bpy.ops.render.render(animation=True, frame_start=9999, frame_end=10000)
bpy.context.scene.render.filepath = 'out/neg'
#@render bpy.ops.render.render(animation=True, frame_start=-12, frame_end=-11)
bpy.context.scene.render.filepath = 'out/n#'
#@render bpy.ops.render.render(animation=True, frame_start=-5, frame_end=-5)
bpy.context.scene.render.filepath = 'out/e.'
#@render bpy.ops.render.render(animation=True, frame_start=7, frame_end=7)
bpy.context.scene.render.filepath = 'out/q#.png'
#@render bpy.ops.render.render(animation=True, frame_start=8, frame_end=8)
bpy.context.scene.render.image_settings.file_format = 'JPEG'
bpy.context.scene.render.filepath = 'out/jpg_###'
#@render bpy.ops.render.render(animation=True, frame_start=0, frame_end=1)
bpy.context.scene.render.image_settings.file_format = 'PNG'
bpy.context.scene.render.use_file_extension = False
bpy.context.scene.render.filepath = 'out/noext_'
#@render bpy.ops.render.render(animation=True, frame_start=1, frame_end=1)
bpy.context.scene.render.use_file_extension = True
bpy.context.scene.render.filepath = 'out/still_###'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.cursor.location = (bpy.context.scene.frame_current, bpy.context.scene.frame_start, bpy.context.scene.frame_end)
