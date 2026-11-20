# golden: startup=factory compare=objects,context tier=T0
# render-output: out/o_rgb.jpg out/o_bw.jpg out/o_rgb.hdr out/o_bw.hdr out/o_rgb.exr out/o_bw.bmp out/o_bw.tga out/o_rgb.webp out/o_agx.png
# Output formats on an opaque film (WP 5.4): the factory scene through Workbench, written with
# render.render(write_still=True) as JPEG (RGB and BW, a quality below 10 that Blender writes at
# 90), Radiance HDR (RGB and BW), OpenEXR (RGB, ZIP), BMP and Targa (BW), WebP and PNG with the
# default AgX view. `#@render` lines run in crates/frap-ops-render-tests
# (tests/render_output.rs).
import bpy
bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.render.resolution_x = 96
bpy.context.scene.render.resolution_y = 54
bpy.context.scene.view_settings.view_transform = 'Standard'
bpy.context.scene.render.image_settings.file_format = 'JPEG'
bpy.context.scene.render.image_settings.quality = 5
bpy.context.scene.render.filepath = 'out/o_rgb'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.render.image_settings.color_mode = 'BW'
bpy.context.scene.render.filepath = 'out/o_bw'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.render.image_settings.file_format = 'HDR'
bpy.context.scene.render.image_settings.color_mode = 'RGB'
bpy.context.scene.render.filepath = 'out/o_rgb'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.render.image_settings.color_mode = 'BW'
bpy.context.scene.render.filepath = 'out/o_bw'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.render.image_settings.file_format = 'OPEN_EXR'
bpy.context.scene.render.image_settings.color_mode = 'RGB'
bpy.context.scene.render.image_settings.exr_codec = 'ZIP'
bpy.context.scene.render.filepath = 'out/o_rgb'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.render.image_settings.file_format = 'BMP'
bpy.context.scene.render.image_settings.color_mode = 'BW'
bpy.context.scene.render.filepath = 'out/o_bw'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.render.image_settings.file_format = 'TARGA'
bpy.context.scene.render.filepath = 'out/o_bw'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.render.image_settings.file_format = 'WEBP'
bpy.context.scene.render.image_settings.color_mode = 'RGB'
bpy.context.scene.render.image_settings.quality = 100
bpy.context.scene.render.filepath = 'out/o_rgb'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.view_transform = 'AgX'
bpy.context.scene.render.image_settings.file_format = 'PNG'
bpy.context.scene.render.image_settings.color_mode = 'RGB'
bpy.context.scene.render.filepath = 'out/o_agx'
#@render bpy.ops.render.render(write_still=True)
