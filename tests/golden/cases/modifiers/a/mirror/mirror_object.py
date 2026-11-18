# WP 4.2: MIRROR about a rotated, moved empty (`mirror_object`): the mirror transform goes
# through the empty's space; the bisect plane follows it and accounts for the object's
# non-uniform scale (#87592).
# golden: startup=factory compare=evaluated:Cube,evaluated:Cube.001 tier=T1
import bpy
bpy.ops.object.duplicate()
bpy.ops.object.empty_add(location=(1.5, 0.2, 0), rotation=(0, 0.3, 0.6))
bpy.data.objects['Cube'].location = (0.5, 0, 0)
bpy.data.objects['Cube'].modifiers.new(name='Mirror', type='MIRROR')
bpy.data.objects['Cube'].modifiers['Mirror'].mirror_object = bpy.data.objects['Empty']
bpy.data.objects['Cube.001'].scale = (1, 2, 0.5)
bpy.data.objects['Cube.001'].rotation_euler = (0.2, 0, 0)
bpy.data.objects['Cube.001'].modifiers.new(name='Mirror', type='MIRROR')
bpy.data.objects['Cube.001'].modifiers['Mirror'].mirror_object = bpy.data.objects['Empty']
bpy.data.objects['Cube.001'].modifiers['Mirror'].use_bisect_axis[0] = True
bpy.data.objects['Cube.001'].modifiers['Mirror'].use_axis = (True, False, True)
bpy.data.objects['Cube.001'].modifiers['Mirror'].use_bisect_axis[2] = True
