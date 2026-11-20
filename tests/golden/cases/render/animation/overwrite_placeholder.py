# golden: startup=factory compare=objects,context tier=T0
# render-animation: files
# render.use_overwrite and render.use_placeholder in animation renders (WP 6.6, RE_RenderAnim):
# with use_overwrite off a frame whose file exists is skipped (not rendered); when every frame is
# skipped the render still succeeds (INFO "No frames rendered, skipped to not overwrite"); still
# renders overwrite regardless. With use_placeholder an empty file is created before each frame
# renders and removed again when the frame fails while it is still empty (a one-channel WebP,
# which Blender cannot save: the error names the path as computed, relative and normalised).
# A directory that cannot be created (under a file) fails as Blender's save does: `Render error
# (<strerror>) cannot save: '<path>'`, with or without the placeholder, still or not. Every file
# written is compared with Blender's (names, empty or not). `#@render` lines run in
# crates/frap-ops-render-tests (tests/render_animation.rs).
import bpy
bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.render.resolution_x = 16
bpy.context.scene.render.resolution_y = 8
bpy.context.scene.render.filepath = 'out/p_'
#@render bpy.ops.render.render(animation=True, frame_start=2, frame_end=2)
bpy.context.scene.render.use_overwrite = False
#@render bpy.ops.render.render(animation=True, frame_start=1, frame_end=3)
#@render bpy.ops.render.render(animation=True, frame_start=1, frame_end=3)
bpy.context.scene.render.filepath = 'out/p_0002'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.render.use_overwrite = True
bpy.context.scene.render.use_placeholder = True
bpy.context.scene.render.image_settings.file_format = 'WEBP'
bpy.context.scene.render.image_settings.color_mode = 'BW'
bpy.context.scene.render.filepath = 'out/w_'
#@render bpy.ops.render.render(animation=True, frame_start=1, frame_end=2)
bpy.context.scene.render.filepath = 'out/sub/../w2_'
#@render bpy.ops.render.render(animation=True, frame_start=1, frame_end=1)
bpy.context.scene.render.image_settings.file_format = 'PNG'
bpy.context.scene.render.filepath = 'out/ph/f_'
#@render bpy.ops.render.render(animation=True, frame_start=1, frame_end=2)
bpy.context.scene.render.use_overwrite = False
#@render bpy.ops.render.render(animation=True, frame_start=2, frame_end=3)
bpy.context.scene.render.use_overwrite = True
bpy.context.scene.render.filepath = 'out/p_0002.png/x_'
#@render bpy.ops.render.render(animation=True, frame_start=1, frame_end=1)
bpy.context.scene.render.use_placeholder = False
#@render bpy.ops.render.render(animation=True, frame_start=1, frame_end=1)
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.cursor.location = (bpy.context.scene.frame_current, bpy.context.scene.frame_start, bpy.context.scene.frame_end)
