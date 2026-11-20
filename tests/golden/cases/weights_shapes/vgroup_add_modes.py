# WP 6.3: `VertexGroup.add` with REPLACE / ADD / SUBTRACT, `weight()` and its errors, group
# naming and the active index. Weights are compared by copying them into the camera's location
# (`objects`) and through a shape key blended with the group (`evaluated:Cube`, the key moves
# every vertex by +1 on Z times its weight).
# golden: startup=factory tier=T0 compare=objects,evaluated:Cube
import bpy
bpy.data.objects['Cube'].vertex_groups.active_index
bpy.data.objects['Cube'].vertex_groups.active
bpy.data.objects['Cube'].vertex_groups.new()
bpy.data.objects['Cube'].vertex_groups.new(name='Group')
bpy.data.objects['Cube'].vertex_groups.new(name='')
bpy.data.objects['Camera'].vertex_groups.new()
bpy.data.objects['Cube'].vertex_groups.active_index
#@blender assert bpy.data.objects['Cube'].vertex_groups.active_index == 2
#@blender assert bpy.data.objects['Cube'].vertex_groups.keys() == ['Group', 'Group.001', 'Group.002']
bpy.data.objects['Cube'].vertex_groups[1].name = 'Group'
#@blender assert bpy.data.objects['Cube'].vertex_groups[1].name == 'Group.001'
bpy.data.objects['Cube'].vertex_groups[0].weight(0)
bpy.data.objects['Cube'].vertex_groups[0].add([0, 1, 2, 5], 0.5, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups[0].weight(0)
bpy.data.objects['Cube'].vertex_groups[0].weight(3)
bpy.data.objects['Cube'].vertex_groups[0].weight(100)
bpy.data.objects['Cube'].vertex_groups[0].add([0, 3], 0.7, 'ADD')
bpy.data.objects['Cube'].vertex_groups[0].add([1, 4], 0.2, 'SUBTRACT')
bpy.data.objects['Cube'].vertex_groups[0].add([2], 0.5, 'SUBTRACT')
bpy.data.objects['Cube'].vertex_groups[0].add([5], 2.0, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups[0].add([6, 100], 0.25, 'ADD')
bpy.data.objects['Cube'].vertex_groups[0].add([6], 0.5, 'FOO')
bpy.data.objects['Cube'].vertex_groups[0].add([6], 0.5)
bpy.data.objects['Cube'].vertex_groups[0].weight(1)
bpy.data.objects['Cube'].vertex_groups[0].weight(2)
bpy.data.objects['Cube'].vertex_groups[0].weight(4)
bpy.data.objects['Cube'].vertex_groups[1].add([7], 1.0, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups[0].remove([7])
bpy.data.objects['Cube'].vertex_groups[1].remove([7])
bpy.data.objects['Cube'].vertex_groups[1].weight(7)
bpy.data.objects['Camera'].location = (bpy.data.objects['Cube'].vertex_groups[0].weight(0), bpy.data.objects['Cube'].vertex_groups[0].weight(1), bpy.data.objects['Cube'].vertex_groups[0].weight(3))
bpy.data.objects['Light'].location = (bpy.data.objects['Cube'].vertex_groups[0].weight(5), bpy.data.objects['Cube'].vertex_groups[0].weight(6), bpy.data.objects['Cube'].vertex_groups.active_index)
bpy.data.objects['Cube'].shape_key_add(name='Basis')
bpy.data.objects['Cube'].shape_key_add(name='Up', from_mix=False)
bpy.data.meshes['Cube'].shape_keys.key_blocks['Up'].data.foreach_set('co', [1, 1, 2, 1, 1, 0, 1, -1, 2, 1, -1, 0, -1, 1, 2, -1, 1, 0, -1, -1, 2, -1, -1, 0])
bpy.data.meshes['Cube'].shape_keys.key_blocks['Up'].vertex_group = 'Group'
bpy.data.meshes['Cube'].shape_keys.key_blocks['Up'].value = 1.0
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.objects['Cube'].vertex_groups[0].add([0], 1.0, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups[0].remove([0])
bpy.data.objects['Cube'].vertex_groups[0].weight(2)
bpy.data.objects['Cube'].vertex_groups.new(name='E')
bpy.ops.object.mode_set(mode='OBJECT')
