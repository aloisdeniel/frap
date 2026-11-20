# golden: startup=factory compare=objects tier=T0
# M5+M6 stabilization: `ImageFormatSettings` setters (`BKE_image_format_set`,
# `BKE_image_format_media_type_set`) and their per-format items. Refused values are TypeErrors
# listing the current items, which shows what the previous line changed: the file format
# offered by each media type, the colour modes and depths of each format.
import bpy
bpy.context.scene.render.image_settings.file_format = 'OPEN_EXR_MULTILAYER'
bpy.context.scene.render.image_settings.file_format = 'FFMPEG'
bpy.context.scene.render.image_settings.file_format = 'CINEON'
bpy.context.scene.render.image_settings.color_depth = '8'
bpy.context.scene.render.image_settings.color_mode = 'RGBA'
bpy.context.scene.render.image_settings.color_mode = 'BW'
bpy.context.scene.render.image_settings.file_format = 'TIFF'
bpy.context.scene.render.image_settings.color_depth = '10'
bpy.context.scene.render.image_settings.color_depth = '16'
bpy.context.scene.render.image_settings.color_mode = 'RGBA'
bpy.context.scene.render.image_settings.file_format = 'IRIS'
bpy.context.scene.render.image_settings.color_depth = '16'
bpy.context.scene.render.image_settings.file_format = 'JPEG'
bpy.context.scene.render.image_settings.color_mode = 'RGBA'
bpy.context.scene.render.image_settings.file_format = 'DPX'
bpy.context.scene.render.image_settings.color_depth = '32'
bpy.context.scene.render.image_settings.color_depth = '12'
bpy.context.scene.render.image_settings.file_format = 'HDR'
bpy.context.scene.render.image_settings.color_depth = '8'
bpy.context.scene.render.image_settings.media_type = 'MULTI_LAYER_IMAGE'
bpy.context.scene.render.image_settings.file_format = 'PNG'
bpy.context.scene.render.image_settings.color_depth = '8'
bpy.context.scene.render.image_settings.color_mode = 'BW'
bpy.context.scene.render.image_settings.media_type = 'IMAGE'
bpy.context.scene.render.image_settings.color_depth = '32'
bpy.context.scene.render.image_settings.file_format = 'AVIF'
bpy.context.scene.render.image_settings.color_depth = '16'
bpy.context.scene.render.image_settings.file_format = 'JPEG2000'
bpy.context.scene.render.image_settings.color_depth = '10'
bpy.context.scene.render.image_settings.file_format = 'BMP'
bpy.context.scene.render.image_settings.color_depth = '16'
bpy.data.objects['Cube'].location.x = 1.0
