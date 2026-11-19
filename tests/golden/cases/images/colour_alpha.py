# WP 3.2: colour space and alpha mode of image files (`imb_handle_colorspace_and_alpha`: a
# 16-bit PNG read as sRGB, Non-Color, CHANNEL_PACKED, NONE; 8-bit PREMUL/NONE), float images
# with partial alpha written to 16-bit PNG and OpenEXR (read back raw as Non-Color), the packed
# float round trip (OpenEXR packed from memory, STRAIGHT: premultiplied on reload), colour space
# and alpha mode changes freeing modified pixels, and the state after `save()` frees the buffer.
# golden: startup=factory tier=T4 tol=1e-4 compare=objects
import bpy
bpy.data.images.new('FA', 2, 1, float_buffer=True, alpha=True)
bpy.data.images['FA'].pixels = [0.1, 0.2, 0.05, 0.5, 0.5, 0.04, 0.9, 0.25]
bpy.data.images['FA'].save(filepath='out/fa.png', save_copy=True)
bpy.data.images['FA'].file_format = 'OPEN_EXR'
bpy.data.images['FA'].save(filepath='out/fa.exr', save_copy=True)
bpy.data.images.load('out/fa.png')
bpy.data.objects.new('PngSRGB', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['PngSRGB'])
bpy.data.objects['PngSRGB'].location = bpy.data.images['fa.png'].pixels[0:3]
bpy.data.objects['PngSRGB'].rotation_euler = bpy.data.images['fa.png'].pixels[3:6]
bpy.data.objects['PngSRGB'].scale = bpy.data.images['fa.png'].pixels[5:8]
bpy.data.objects['PngSRGB'].delta_location = bpy.data.images['fa.png'].pixels[2:5]
bpy.data.images['fa.png'].colorspace_settings.name = 'Non-Color'
bpy.data.objects.new('PngRaw', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['PngRaw'])
bpy.data.objects['PngRaw'].location = bpy.data.images['fa.png'].pixels[0:3]
bpy.data.objects['PngRaw'].rotation_euler = bpy.data.images['fa.png'].pixels[3:6]
bpy.data.objects['PngRaw'].scale = bpy.data.images['fa.png'].pixels[5:8]
bpy.data.objects['PngRaw'].delta_location.x = bpy.data.images['fa.png'].depth
bpy.data.objects['PngRaw'].delta_location.y = bpy.data.images['fa.png'].is_float
bpy.data.images['fa.png'].reload()
bpy.data.objects.new('PngRawReload', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['PngRawReload'])
bpy.data.objects['PngRawReload'].location = bpy.data.images['fa.png'].pixels[0:3]
bpy.data.objects['PngRawReload'].rotation_euler = bpy.data.images['fa.png'].pixels[3:6]
bpy.data.objects['PngRawReload'].scale = bpy.data.images['fa.png'].pixels[5:8]
bpy.data.images['fa.png'].alpha_mode = 'CHANNEL_PACKED'
bpy.data.images['fa.png'].colorspace_settings.name = 'sRGB'
bpy.data.objects.new('PngPacked', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['PngPacked'])
bpy.data.objects['PngPacked'].location = bpy.data.images['fa.png'].pixels[0:3]
bpy.data.objects['PngPacked'].rotation_euler = bpy.data.images['fa.png'].pixels[3:6]
bpy.data.objects['PngPacked'].scale = bpy.data.images['fa.png'].pixels[5:8]
bpy.data.images['fa.png'].alpha_mode = 'NONE'
bpy.data.objects.new('PngNone', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['PngNone'])
bpy.data.objects['PngNone'].location = bpy.data.images['fa.png'].pixels[0:3]
bpy.data.objects['PngNone'].rotation_euler = bpy.data.images['fa.png'].pixels[3:6]
bpy.data.objects['PngNone'].scale = bpy.data.images['fa.png'].pixels[5:8]
bpy.data.images['fa.png'].colorspace_settings.is_data = True
bpy.data.objects.new('PngNoneData', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['PngNoneData'])
bpy.data.objects['PngNoneData'].location = bpy.data.images['fa.png'].pixels[0:3]
bpy.data.objects['PngNoneData'].rotation_euler = bpy.data.images['fa.png'].pixels[3:6]
bpy.data.objects['PngNoneData'].scale = bpy.data.images['fa.png'].pixels[5:8]
bpy.data.images.load('out/fa.exr')
bpy.data.images['fa.exr'].colorspace_settings.name = 'Non-Color'
bpy.data.objects.new('ExrRaw', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['ExrRaw'])
bpy.data.objects['ExrRaw'].location = bpy.data.images['fa.exr'].pixels[0:3]
bpy.data.objects['ExrRaw'].rotation_euler = bpy.data.images['fa.exr'].pixels[3:6]
bpy.data.objects['ExrRaw'].scale = bpy.data.images['fa.exr'].pixels[5:8]
bpy.data.objects['ExrRaw'].delta_location.x = bpy.data.images['fa.exr'].depth
bpy.data.images.new('B', 2, 1, alpha=True)
bpy.data.images['B'].pixels = [0.2, 0.4, 0.6, 0.5, 1.0, 0.5, 0.25, 0.25]
bpy.data.images['B'].save(filepath='out/b.png', save_copy=True)
bpy.data.images.load('out/b.png')
bpy.data.images['b.png'].alpha_mode = 'PREMUL'
bpy.data.objects.new('BytePremul', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['BytePremul'])
bpy.data.objects['BytePremul'].location = bpy.data.images['b.png'].pixels[0:3]
bpy.data.objects['BytePremul'].rotation_euler = bpy.data.images['b.png'].pixels[3:6]
bpy.data.objects['BytePremul'].scale = bpy.data.images['b.png'].pixels[5:8]
bpy.data.images['b.png'].alpha_mode = 'NONE'
bpy.data.objects.new('ByteNone', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['ByteNone'])
bpy.data.objects['ByteNone'].location = bpy.data.images['b.png'].pixels[0:3]
bpy.data.objects['ByteNone'].rotation_euler = bpy.data.images['b.png'].pixels[3:6]
bpy.data.objects['ByteNone'].scale = bpy.data.images['b.png'].pixels[5:8]
bpy.data.images.new('PF', 2, 1, float_buffer=True, alpha=True)
bpy.data.images['PF'].pixels = [0.1, 2.5, 0.05, 0.5, 0.3, 0.2, 0.1, 1.0]
bpy.data.images['PF'].pack()
bpy.data.images['PF'].reload()
bpy.data.objects.new('PackedFloat', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['PackedFloat'])
bpy.data.objects['PackedFloat'].location = bpy.data.images['PF'].pixels[0:3]
bpy.data.objects['PackedFloat'].rotation_euler = bpy.data.images['PF'].pixels[3:6]
bpy.data.objects['PackedFloat'].scale = bpy.data.images['PF'].pixels[5:8]
bpy.data.images.load('out/b.png')
bpy.data.images['b.png.001'].pixels[0] = 1.0
bpy.data.images['b.png.001'].colorspace_settings.name = 'Non-Color'
bpy.data.objects.new('FreedCS', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['FreedCS'])
bpy.data.objects['FreedCS'].location = bpy.data.images['b.png.001'].pixels[0:3]
bpy.data.objects['FreedCS'].scale.x = bpy.data.images['b.png.001'].is_dirty
bpy.data.images['b.png.001'].pixels[0] = 1.0
bpy.data.images['b.png.001'].alpha_mode = 'PREMUL'
bpy.data.objects.new('FreedAlpha', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['FreedAlpha'])
bpy.data.objects['FreedAlpha'].location = bpy.data.images['b.png.001'].pixels[0:3]
bpy.data.objects['FreedAlpha'].scale.x = bpy.data.images['b.png.001'].is_dirty
bpy.data.images.new('G', 2, 1)
bpy.data.images['G'].pixels[0] = 0.5
bpy.data.images['G'].alpha_mode = 'PREMUL'
bpy.data.objects.new('GenAlpha', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['GenAlpha'])
bpy.data.objects['GenAlpha'].location = bpy.data.images['G'].pixels[0:3]
bpy.data.objects['GenAlpha'].scale.x = bpy.data.images['G'].is_dirty
bpy.data.images['G'].colorspace_settings.name = 'Linear Rec.709'
bpy.data.objects.new('GenCS', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['GenCS'])
bpy.data.objects['GenCS'].location = bpy.data.images['G'].pixels[0:3]
bpy.data.objects['GenCS'].scale.x = bpy.data.images['G'].is_dirty
bpy.data.images.new('SF', 2, 2, float_buffer=True)
bpy.data.images['SF'].file_format = 'OPEN_EXR'
bpy.data.images['SF'].save(filepath='out/sf.exr')
bpy.data.objects.new('SF', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['SF'])
bpy.data.objects['SF'].location.x = bpy.data.images['SF'].size[0]
bpy.data.objects['SF'].location.y = bpy.data.images['SF'].has_data
bpy.data.objects['SF'].name = bpy.data.images['SF'].file_format
bpy.data.objects.new('SF2', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['SF2'])
bpy.data.objects['SF2'].name = bpy.data.images['SF'].colorspace_settings.name
