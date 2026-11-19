# M3+M4 stabilization round 2: a generated image packs from memory under its buffer's path,
# made by `images.new` before `filepath_raw` was set: the packed file's `filepath` is '' and
# `unpack()` writes `textures/<image name>.png`.
# golden: startup=factory tier=T0 compare=objects,image:textures/Ok.png
import bpy
bpy.data.images.new('Ok', 2, 2)
bpy.data.images['Ok'].pixels[0:4] = (1.0, 0.0, 0.0, 1.0)
bpy.data.images['Ok'].filepath_raw = '/nonexistent/dir/x.png'
bpy.data.images['Ok'].pack()
bpy.data.objects.new('P', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P'])
bpy.data.objects['P'].name = 'path:' + bpy.data.images['Ok'].packed_files[0].filepath
bpy.data.images['Ok'].unpack()
bpy.data.objects.new('F', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['F'])
bpy.data.objects['F'].name = bpy.data.images['Ok'].filepath
