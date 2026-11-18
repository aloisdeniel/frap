# WP 3.2: `Image.save_render()`: the scene's output format and colour management (AgX by default,
# Standard, an RGB colour mode, JPEG) applied to byte and float images; the image is unchanged;
# errors without data.
# golden: startup=factory tier=T4 ssim=0.99 compare=objects,image:out/agx.png,image:out/agx_f.png,image:out/std.png,image:out/rgb.png
import bpy
bpy.data.images.new('R', 3, 2, alpha=True)
bpy.data.images['R'].pixels = [0.2, 0.5, 0.8, 1.0, 0.9, 0.05, 0.3, 0.5, 1.0, 0.0, 0.01, 0.25, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.5, 0.5, 0.5, 0.0]
bpy.data.images.new('RF', 3, 2, float_buffer=True, alpha=True)
bpy.data.images['RF'].pixels = [0.2, 0.5, 0.8, 1.0, 0.45, 0.025, 0.15, 0.5, 1.5, 0.0, 0.01, 0.25, 0.0, 0.0, 0.0, 1.0, 4.0, 2.0, 0.18, 1.0, 0.05, 0.05, 0.05, 0.1]
bpy.data.images['R'].save_render('out/agx.png')
bpy.data.images['RF'].save_render('out/agx_f.png', scene=bpy.data.scenes['Scene'])
bpy.context.scene.view_settings.view_transform = 'Standard'
bpy.data.images['RF'].save_render('out/std.png')
bpy.context.scene.render.image_settings.color_mode = 'RGB'
bpy.data.images['R'].save_render(filepath='out/rgb.png')
bpy.context.scene.render.image_settings.file_format = 'JPEG'
bpy.data.images['R'].save_render('out/r.jpg', quality=50)
bpy.data.objects.new('R', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['R'])
bpy.data.objects['R'].location.x = bpy.data.images['R'].is_dirty
bpy.data.objects['R'].location.y = bpy.data.images['R'].pixels[1]
bpy.data.objects['R'].name = bpy.data.images['R'].source
bpy.data.images['R'].save_render('r.png')
bpy.data.images['RF'].save_render('out/r.exr')
