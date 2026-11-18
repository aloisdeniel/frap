# WP 3.2: images through save_as_mainfile / open_mainfile: packed images keep their bytes (packed
# from memory or from a file), generated images keep their settings (depth, colour, type), and
# unpacked pixel edits are lost (generated again), as in Blender.
# Paths are in the work directory: Blender's save does not create directories.
# golden: startup=factory tier=T0 compare=objects
import bpy
bpy.data.images.new('P', 3, 2)
bpy.data.images['P'].pixels[0:4] = (1.0, 0.5, 0.0, 1.0)
bpy.data.images['P'].pack()
bpy.data.images['P'].use_fake_user = True
bpy.data.images.new('G', 2, 2, alpha=True)
bpy.data.images['G'].generated_color = (0.25, 0.5, 0.75, 1.0)
bpy.data.images['G'].pixels[0:4] = (1.0, 1.0, 1.0, 1.0)
bpy.data.images['G'].use_fake_user = True
bpy.ops.image.new(name='U', width=40, height=36, generated_type='UV_GRID', float=True)
bpy.data.images['U'].use_fake_user = True
bpy.data.images.new('F', 2, 1)
bpy.data.images['F'].pixels[0:4] = (0.0, 0.0, 1.0, 1.0)
bpy.data.images['F'].save(filepath='out/f.png')
bpy.data.images.load('out/f.png')
bpy.data.images['f.png'].pack()
bpy.data.images['f.png'].use_fake_user = True
bpy.ops.wm.save_as_mainfile(filepath='img.frap')
bpy.ops.wm.open_mainfile(filepath='img.frap')
bpy.data.objects['Cube'].location = bpy.data.images['P'].pixels[0:3]
bpy.data.objects['Cube'].scale = bpy.data.images['G'].pixels[0:3]
bpy.data.objects['Cube'].rotation_euler.x = bpy.data.images['G'].depth
bpy.data.objects['Cube'].rotation_euler.y = bpy.data.images['G'].is_dirty
bpy.data.objects['Cube'].rotation_euler.z = bpy.data.images['P'].size[0]
bpy.data.objects['Camera'].location = bpy.data.images['f.png'].pixels[0:3]
bpy.data.objects['Camera'].scale = bpy.data.images['U'].pixels[2624:2627]
bpy.data.objects['Camera'].rotation_euler.x = bpy.data.images['U'].depth
bpy.data.objects['Camera'].name = bpy.data.images['P'].source
bpy.data.objects['Light'].name = bpy.data.images['f.png'].packed_files[0].filepath
bpy.data.images['f.png'].unpack(method='WRITE_LOCAL')
