# WP 3.2: `Image.save()` (formats from the buffer, colour mode, copy, errors, the state after a
# save), `bpy.data.images.load()` (metadata read from the file, check_existing, errors) and
# `image.open`. The written files are compared pixel by pixel (SSIM); file metadata and
# states are copied into empties.
# golden: startup=factory tier=T4 ssim=0.999 tol=1e-4 compare=objects,image:out/a.png,image:out/grid.png,image:out/f.png,image:out/copy.png,image:out/scaled.png
import bpy
bpy.data.images.new('A', 4, 2, alpha=True)
bpy.data.images['A'].pixels = [0.0, 0.1, 0.2, 1.0, 0.3, 0.4, 0.5, 0.5, 0.6, 0.7, 0.8, 1.0, 0.9, 1.0, 0.0, 0.25, 1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.5, 0.5, 0.5, 0.0]
bpy.data.images['A'].save()
bpy.data.images['A'].save(filepath='a.png')
bpy.data.images['A'].save(filepath='out/a.png')
bpy.data.objects.new('A', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['A'])
bpy.data.objects['A'].location.x = bpy.data.images['A'].is_dirty
bpy.data.objects['A'].location.y = bpy.data.images['A'].pixels[1]
bpy.data.objects['A'].location.z = bpy.data.images['A'].size[0]
bpy.data.objects['A'].name = bpy.data.images['A'].source
bpy.data.images.new('Copy', 3, 3)
bpy.data.images['Copy'].pixels[0:4] = (1.0, 0.0, 0.0, 1.0)
bpy.data.images['Copy'].save(filepath='out/copy.png', save_copy=True)
bpy.data.objects.new('Copy', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Copy'])
bpy.data.objects['Copy'].location.x = bpy.data.images['Copy'].is_dirty
bpy.data.objects['Copy'].name = bpy.data.images['Copy'].source
bpy.ops.image.new(name='Grid', width=64, height=48, generated_type='UV_GRID')
bpy.data.images['Grid'].save(filepath='out/grid.png')
bpy.data.images.new('F', 3, 2, float_buffer=True)
bpy.data.images['F'].generated_color = (0.5, 0.25, 1.0, 1.0)
bpy.data.images['F'].save(filepath='out/f.png')
bpy.data.objects.new('F', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['F'])
bpy.data.objects['F'].location.x = bpy.data.images['F'].size[0]
bpy.data.objects['F'].location.y = bpy.data.images['F'].has_data
bpy.data.objects['F'].name = bpy.data.images['F'].colorspace_settings.name
bpy.data.images.new('S', 5, 4)
bpy.data.images['S'].pixels[0:8] = (1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 1.0, 1.0)
bpy.data.images['S'].scale(10, 3)
bpy.data.images['S'].save(filepath='out/scaled.png')
bpy.data.images.load('out/a.png')
bpy.data.images.load('out/a.png')
bpy.data.images.load('out/a.png', check_existing=True)
bpy.data.objects.new('L', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['L'])
bpy.data.objects['L'].location.x = bpy.data.images['a.png'].size[0]
bpy.data.objects['L'].location.y = bpy.data.images['a.png'].depth
bpy.data.objects['L'].location.z = bpy.data.images['a.png'].users
bpy.data.objects['L'].scale = bpy.data.images['a.png.001'].pixels[4:7]
bpy.data.objects['L'].rotation_euler.x = bpy.data.images['a.png'].pixels[7]
bpy.data.objects['L'].rotation_euler.y = bpy.data.images['a.png'].channels
bpy.data.objects['L'].rotation_euler.z = bpy.data.images['a.png'].is_float
bpy.data.objects['L'].name = bpy.data.images['a.png'].filepath
bpy.data.objects.new('L2', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['L2'])
bpy.data.objects['L2'].name = bpy.data.images['a.png.001'].file_format
bpy.data.objects.new('L3', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['L3'])
bpy.data.objects['L3'].name = bpy.data.images['a.png.001'].alpha_mode
bpy.data.images.load('out/f.png')
bpy.data.objects.new('L4', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['L4'])
bpy.data.objects['L4'].location.x = bpy.data.images['f.png'].depth
bpy.data.objects['L4'].location.y = bpy.data.images['f.png'].is_float
bpy.data.objects['L4'].scale = bpy.data.images['f.png'].pixels[0:3]
bpy.data.images.load('out/missing.png')
bpy.data.images.load('out')
bpy.data.objects.new('L5', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['L5'])
bpy.data.objects['L5'].location.x = bpy.data.images['out'].size[0]
bpy.data.objects['L5'].location.y = bpy.data.images['out'].has_data
bpy.data.images['out'].save(filepath='out/none.png')
bpy.data.images['out'].scale(2, 2)
bpy.data.images['out'].update()
bpy.ops.image.open(filepath='out/grid.png')
bpy.ops.image.open(filepath='out/grid.png')
bpy.ops.image.open(filepath='out/nope.png')
bpy.data.objects.new('O', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['O'])
bpy.data.objects['O'].location.x = bpy.data.images['grid.png'].size[1]
bpy.data.objects['O'].location.y = bpy.data.images['grid.png'].depth
bpy.data.objects['O'].scale = bpy.data.images['grid.png'].pixels[2624:2627]
bpy.data.images['grid.png.001'].name
bpy.data.images['grid.png'].filepath_from_user()
