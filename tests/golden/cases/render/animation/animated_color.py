# golden: startup=factory compare=context tier=T0
# render-animation: files needs=animation images=out/cm_0001.png,out/cm_0002.png,out/cm_0004.png
# M5+M6 stabilization round 7: render.render(animation=True) writes each frame with the colour
# management of that frame's scene (`BKE_image_render_write` reads `scene->view_settings` after
# `RE_RenderAnim` evaluated the scene's animation): exposure keyed 0 -> 2 and gamma 1 -> 0.6
# over frames 1..4, and dither_intensity keyed 1 -> 0 (Workbench renders, compared with
# Blender's files).
import bpy
bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.render.resolution_x = 96
bpy.context.scene.render.resolution_y = 54
bpy.context.scene.frame_start = 1
bpy.context.scene.frame_end = 4
bpy.context.scene.view_settings.exposure = 0.0
bpy.context.scene.view_settings.keyframe_insert('exposure', frame=1)
bpy.context.scene.view_settings.exposure = 2.0
bpy.context.scene.view_settings.keyframe_insert('exposure', frame=4)
bpy.context.scene.view_settings.gamma = 1.0
bpy.context.scene.view_settings.keyframe_insert('gamma', frame=1)
bpy.context.scene.view_settings.gamma = 0.6
bpy.context.scene.view_settings.keyframe_insert('gamma', frame=4)
bpy.context.scene.render.dither_intensity = 1.0
bpy.context.scene.render.keyframe_insert('dither_intensity', frame=1)
bpy.context.scene.render.dither_intensity = 0.0
bpy.context.scene.render.keyframe_insert('dither_intensity', frame=4)
bpy.context.scene.frame_current = 1
bpy.context.scene.render.filepath = 'out/cm_'
#@render bpy.ops.render.render(animation=True)
bpy.context.scene.cursor.location = (bpy.context.scene.frame_current, bpy.context.scene.view_settings.exposure, bpy.context.scene.view_settings.gamma)
