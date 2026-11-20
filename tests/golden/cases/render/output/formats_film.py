# golden: startup=factory compare=objects,context tier=T0
# render-output: out/f_rgba.tif out/f_bw16.tif out/f_rgb.rgb out/f_rgba10.dpx out/f_bw16.dpx out/f_rgb.cin out/f_ml.exr out/f_ml16.exr
# Output formats of the M5+M6 stabilization: the factory scene through Workbench with a
# transparent film, written with render.render(write_still=True) as TIFF (8-bit RGBA deflate,
# 16-bit BW LZW), IRIS (RGB), DPX (10-bit RGBA, 16-bit BW, scene-linear), Cineon (RGB, printing
# density) and multilayer OpenEXR (32- and 16-bit: the Combined pass whatever color_mode
# says). `#@render` lines run in crates/frap-ops-render-tests (tests/render_output.rs); the
# exact bytes and samples of every writer are checked on one film by frap-ops-render's
# `output::format` tests.
import bpy
bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.render.resolution_x = 96
bpy.context.scene.render.resolution_y = 54
bpy.context.scene.render.film_transparent = True
bpy.context.scene.view_settings.view_transform = 'Standard'
bpy.context.scene.render.image_settings.file_format = 'TIFF'
bpy.context.scene.render.image_settings.color_mode = 'RGBA'
bpy.context.scene.render.filepath = 'out/f_rgba'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.render.image_settings.color_mode = 'BW'
bpy.context.scene.render.image_settings.color_depth = '16'
bpy.context.scene.render.image_settings.tiff_codec = 'LZW'
bpy.context.scene.render.filepath = 'out/f_bw16'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.render.image_settings.file_format = 'IRIS'
bpy.context.scene.render.image_settings.color_mode = 'RGB'
bpy.context.scene.render.filepath = 'out/f_rgb'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.render.image_settings.file_format = 'DPX'
bpy.context.scene.render.image_settings.color_mode = 'RGBA'
bpy.context.scene.render.image_settings.color_depth = '10'
bpy.context.scene.render.filepath = 'out/f_rgba10'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.render.image_settings.color_mode = 'BW'
bpy.context.scene.render.image_settings.color_depth = '16'
bpy.context.scene.render.filepath = 'out/f_bw16'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.render.image_settings.file_format = 'CINEON'
bpy.context.scene.render.image_settings.color_mode = 'RGB'
bpy.context.scene.render.filepath = 'out/f_rgb'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.render.image_settings.media_type = 'MULTI_LAYER_IMAGE'
bpy.context.scene.render.image_settings.color_depth = '32'
bpy.context.scene.render.filepath = 'out/f_ml'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.render.image_settings.color_depth = '16'
bpy.context.scene.render.image_settings.color_mode = 'BW'
bpy.context.scene.render.filepath = 'out/f_ml16'
#@render bpy.ops.render.render(write_still=True)
