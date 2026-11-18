# WP 3.2: `Image.pack()` (from memory for a modified image, from the file otherwise, nothing for
# an unmodified generated image), `packed_file`/`packed_files`, `unpack()` with every method,
# `ImagePackedFile.save()` and `reload()` of packed images. States are copied into empties.
# golden: startup=factory tier=T0 compare=objects,image:textures/A.png,image:out/b2.png
import bpy
bpy.data.images.new('Fresh', 2, 2)
bpy.data.images['Fresh'].pack()
bpy.data.images['Fresh'].packed_file.size
bpy.data.images['Fresh'].unpack()
bpy.data.images.new('A', 3, 2)
bpy.data.images['A'].pixels[0:4] = (1.0, 0.5, 0.0, 1.0)
bpy.data.images['A'].pack()
bpy.data.objects.new('A', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['A'])
bpy.data.objects['A'].location.x = bpy.data.images['A'].is_dirty
bpy.data.objects['A'].location.y = bpy.data.images['A'].packed_files[0].tile_number
bpy.data.objects['A'].location.z = bpy.data.images['A'].pixels[1]
bpy.data.objects['A'].name = bpy.data.images['A'].source
bpy.data.objects.new('A2', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['A2'])
bpy.data.objects['A2'].name = bpy.data.images['A'].type
bpy.data.images['A'].pack()
bpy.data.images['A'].packed_files[0].save()
bpy.data.images['A'].unpack(method='WRITE_LOCAL')
bpy.data.objects.new('A3', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['A3'])
bpy.data.objects['A3'].name = bpy.data.images['A'].filepath
bpy.data.images['A'].unpack()
bpy.data.images.new('B', 2, 1)
bpy.data.images['B'].pixels[0:4] = (0.0, 0.0, 1.0, 1.0)
bpy.data.images['B'].save(filepath='out/b.png')
bpy.data.images.load('out/b.png')
bpy.data.images['b.png'].pack()
bpy.data.objects.new('B', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['B'])
bpy.data.objects['B'].name = bpy.data.images['b.png'].packed_files[0].filepath
bpy.data.images['b.png'].pixels[0:4] = (0.0, 1.0, 0.0, 1.0)
bpy.data.images['b.png'].reload()
bpy.data.objects.new('B2', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['B2'])
bpy.data.objects['B2'].location = bpy.data.images['b.png'].pixels[0:3]
bpy.data.objects['B2'].scale.x = bpy.data.images['b.png'].is_dirty
bpy.data.images['b.png'].unpack(method='REMOVE')
bpy.data.objects['B2'].scale.y = bpy.data.images['b.png'].packed_files[0].view
bpy.data.images['b.png'].pack()
bpy.data.images['b.png'].unpack(method='USE_ORIGINAL')
bpy.data.images['b.png'].pack()
bpy.data.images['b.png'].unpack(method='USE_LOCAL')
bpy.data.objects.new('B3', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['B3'])
bpy.data.objects['B3'].name = bpy.data.images['b.png'].filepath
bpy.data.images['b.png'].pack()
bpy.data.images['b.png'].unpack(method='WRITE_ORIGINAL')
bpy.data.images.load('out/b.png').save(filepath='out/b2.png')
bpy.data.images.load('out/b.png')
bpy.data.images['b.png.002'].filepath = 'out/missing.png'
bpy.data.images['b.png.002'].pack()
bpy.data.images['b.png.002'].unpack(method='BOGUS')
