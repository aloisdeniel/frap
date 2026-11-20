# golden: startup=factory compare=objects,context tier=T0
# render-output: out/t_rgba8.png out/t_rgb8.png out/t_bw16.png out/t_rgba16.png out/t_half.exr out/t_bw.exr out/t_rle.tga out/t_raw.tga out/t_rgba.bmp out/t_rgba.webp
# Output formats with film_transparent (WP 5.4): the factory cube through Workbench on a
# transparent film, written with render.render(write_still=True) in every format that keeps
# alpha, and in RGB and BW (composited over black). `#@render` lines run in
# crates/frap-ops-render-tests (tests/render_output.rs, they need a GPU), which compares every
# file with Blender's (format, size, channels, bit depth, transparent background, SSIM); the
# generic golden suite compares the scene state and that the settings are accepted.
import bpy
bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.render.film_transparent = True
bpy.context.scene.render.resolution_x = 96
bpy.context.scene.render.resolution_y = 54
bpy.context.scene.view_settings.view_transform = 'Standard'
bpy.context.scene.render.image_settings.file_format = 'PNG'
bpy.context.scene.render.filepath = 'out/t_rgba8'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.render.image_settings.color_mode = 'RGB'
bpy.context.scene.render.filepath = 'out/t_rgb8'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.render.image_settings.color_mode = 'BW'
bpy.context.scene.render.image_settings.color_depth = '16'
bpy.context.scene.render.filepath = 'out/t_bw16'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.render.image_settings.color_mode = 'RGBA'
bpy.context.scene.render.filepath = 'out/t_rgba16'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.render.image_settings.file_format = 'OPEN_EXR'
bpy.context.scene.render.image_settings.color_depth = '16'
bpy.context.scene.render.image_settings.exr_codec = 'PIZ'
bpy.context.scene.render.filepath = 'out/t_half'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.render.image_settings.color_mode = 'BW'
bpy.context.scene.render.image_settings.color_depth = '32'
bpy.context.scene.render.filepath = 'out/t_bw'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.render.image_settings.file_format = 'TARGA'
bpy.context.scene.render.image_settings.color_mode = 'RGBA'
bpy.context.scene.render.filepath = 'out/t_rle'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.render.image_settings.file_format = 'TARGA_RAW'
bpy.context.scene.render.filepath = 'out/t_raw'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.render.image_settings.file_format = 'BMP'
bpy.context.scene.render.image_settings.color_mode = 'RGBA'
bpy.context.scene.render.filepath = 'out/t_rgba'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.render.image_settings.file_format = 'WEBP'
bpy.context.scene.render.image_settings.color_mode = 'RGBA'
bpy.context.scene.render.image_settings.quality = 100
bpy.context.scene.render.filepath = 'out/t_rgba'
#@render bpy.ops.render.render(write_still=True)
