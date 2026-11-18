# WP 4.2: MIRROR UV options on the copy: U mirrored about 0.5 + mirror_offset_u, V mirrored
# around each UDIM tile centre, then offset_u/offset_v added; a second axis (Y) mirrors the
# already mirrored half again.
# golden: startup=factory compare=evaluated:Cube,evaluated:Cube.001 tier=T1
import bpy
bpy.ops.object.duplicate()
bpy.data.objects['Cube'].modifiers.new(name='Mirror', type='MIRROR')
bpy.data.objects['Cube'].modifiers['Mirror'].use_mirror_u = True
bpy.data.objects['Cube'].modifiers['Mirror'].mirror_offset_u = 0.25
bpy.data.objects['Cube'].modifiers['Mirror'].offset_u = 1.5
bpy.data.objects['Cube'].modifiers['Mirror'].offset_v = -0.5
bpy.data.objects['Cube.001'].modifiers.new(name='Mirror', type='MIRROR')
bpy.data.objects['Cube.001'].modifiers['Mirror'].use_axis = (True, True, False)
bpy.data.objects['Cube.001'].modifiers['Mirror'].use_mirror_v = True
bpy.data.objects['Cube.001'].modifiers['Mirror'].use_mirror_udim = True
bpy.data.objects['Cube.001'].modifiers['Mirror'].offset_v = 1.25
