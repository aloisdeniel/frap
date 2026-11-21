# golden: startup=factory compare=objects,context tier=T0
# render-output: out/ov_scene.png
# M5+M6 stabilization round 8: switching image_settings.color_management to OVERRIDE copies the
# scene's colour management into the output settings
# (`rna_ImageFormatSettings_color_management_set` ->
# `BKE_image_format_color_management_copy_from_scene`): the scene's AgX view, look, exposure
# and gamma, its display, and the scene-linear output space. Setting OVERRIDE again copies
# nothing; only the override's exposure is changed afterwards (Workbench FLAT single colour,
# compared with Blender's file).
import bpy
bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.render.resolution_x = 96
bpy.context.scene.render.resolution_y = 54
bpy.context.scene.display.shading.light = 'FLAT'
bpy.context.scene.display.shading.color_type = 'SINGLE'
bpy.context.scene.display.shading.single_color = (0.1, 0.5, 0.9)
bpy.context.scene.view_settings.exposure = 0.7
bpy.context.scene.view_settings.look = 'AgX - Base Contrast'
bpy.context.scene.view_settings.gamma = 1.2
bpy.context.scene.render.image_settings.linear_colorspace_settings.name = 'ACEScg'
#@blender assert bpy.context.scene.render.image_settings.view_settings.view_transform == 'Standard'
bpy.context.scene.render.image_settings.color_management = 'OVERRIDE'
#@blender assert bpy.context.scene.render.image_settings.view_settings.view_transform == 'AgX'
#@blender assert bpy.context.scene.render.image_settings.view_settings.look == 'AgX - Base Contrast'
#@blender assert bpy.context.scene.render.image_settings.linear_colorspace_settings.name == 'Linear Rec.709'
bpy.context.scene.view_settings.exposure = 2.0
bpy.context.scene.render.image_settings.color_management = 'OVERRIDE'
bpy.context.scene.cursor.location = (bpy.context.scene.render.image_settings.view_settings.exposure, bpy.context.scene.render.image_settings.view_settings.gamma, 0.0)
bpy.context.scene.view_settings.exposure = 0.0
bpy.context.scene.render.image_settings.view_settings.exposure = 0.3
bpy.context.scene.render.filepath = 'out/ov_scene'
#@render bpy.ops.render.render(write_still=True)
