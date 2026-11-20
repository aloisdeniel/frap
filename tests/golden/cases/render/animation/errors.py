# golden: startup=factory compare=objects,context tier=T0
# render.render checks of animation renders (WP 6.6), in Blender's order (screen_render_exec):
# frame_start/frame_end set (whatever the value) without animation, then the frame range (the
# operator's frame_start/frame_end override the scene's only when set), then a video format for
# a still, then render.save_output (RE_disable_save_output_allowed), then the size and the
# camera. None of them renders, so the case needs no GPU. (It ends with the camera restored:
# Blender 5.2.4 exits with status 1 when its last render failed for want of a camera.)
import bpy
bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.render.resolution_x = 16
bpy.context.scene.render.resolution_y = 8
bpy.ops.render.render(frame_start=2)
bpy.ops.render.render(frame_end=0)
bpy.ops.render.render(write_still=True, frame_start=1, frame_end=1)
bpy.ops.render.render(frame_start=5, frame_end=4)
bpy.ops.render.render(animation=True, frame_start=5, frame_end=4)
bpy.ops.render.render(animation=True, frame_start=251)
bpy.ops.render.render(animation=True, frame_end=0)
bpy.ops.render.render(animation=True, frame_end=-1, frame_start=0)
bpy.context.scene.render.image_settings.media_type = 'VIDEO'
bpy.ops.render.render(write_still=True, frame_start=1)
bpy.ops.render.render(write_still=True)
bpy.ops.render.render(write_still=True, animation=True, frame_start=3, frame_end=2)
bpy.context.scene.render.image_settings.media_type = 'IMAGE'
bpy.context.scene.render.save_output = False
bpy.ops.render.render(animation=True)
bpy.ops.render.render(animation=True, frame_start=5, frame_end=4)
bpy.context.scene.render.use_compositing = False
bpy.ops.render.render(animation=True)
bpy.context.scene.camera = None
bpy.ops.render.render(animation=True)
bpy.context.scene.render.resolution_percentage = 1
bpy.ops.render.render(animation=True)
bpy.context.scene.render.save_output = True
bpy.ops.render.render(animation=True)
bpy.context.scene.render.resolution_percentage = 100
bpy.ops.render.render(animation=True)
bpy.ops.render.render(frame_start=1, frame_end=1)
bpy.context.scene.camera = bpy.data.objects['Camera']
bpy.context.scene.render.resolution_percentage = 1
bpy.ops.render.render(animation=True)
bpy.context.scene.cursor.location = (bpy.context.scene.frame_current, bpy.context.scene.frame_start, bpy.context.scene.frame_end)
