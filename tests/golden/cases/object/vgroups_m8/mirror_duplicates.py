# WP 8.3.7: `object.vertex_group_mirror` (spatial X mirror) on a mesh whose every vertex has a
# coincident duplicate: the partner of a vertex is the one Blender's KD-tree traversal reaches
# first among the equally near candidates (`kdtree_find_nearest`), not the lowest index. Object
# Mode on the doubled cube, then Edit Mode on a copy with every vertex selected. Weights are
# read through DISPLACE modifiers (G.L along X, G.R along Y).
# golden: startup=factory tier=T1 compare=objects,evaluated:Cube,evaluated:Cube.001
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.duplicate()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Cube'].vertex_groups.new(name='G.L')
bpy.data.objects['Cube'].vertex_groups.new(name='G.R')
bpy.data.objects['Cube'].vertex_groups['G.L'].add([0], 0.1, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups['G.L'].add([1], 0.2, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups['G.L'].add([2], 0.3, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups['G.L'].add([3], 0.4, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups['G.L'].add([8], 0.5, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups['G.L'].add([9], 0.6, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups['G.R'].add([4], 0.7, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups['G.R'].add([5], 0.8, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups['G.R'].add([12], 0.9, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups['G.R'].add([15], 0.15, 'REPLACE')
bpy.ops.object.duplicate()
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.vertex_group_mirror(all_groups=True)
bpy.data.objects['Cube'].modifiers.new(name='A', type='DISPLACE')
bpy.data.objects['Cube'].modifiers['A'].vertex_group = 'G.L'
bpy.data.objects['Cube'].modifiers['A'].direction = 'X'
bpy.data.objects['Cube'].modifiers.new(name='B', type='DISPLACE')
bpy.data.objects['Cube'].modifiers['B'].vertex_group = 'G.R'
bpy.data.objects['Cube'].modifiers['B'].direction = 'Y'
bpy.context.view_layer.objects.active = bpy.data.objects['Cube.001']
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube.001'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.object.vertex_group_mirror(all_groups=True, flip_group_names=False)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Cube.001'].modifiers.new(name='A', type='DISPLACE')
bpy.data.objects['Cube.001'].modifiers['A'].vertex_group = 'G.L'
bpy.data.objects['Cube.001'].modifiers['A'].direction = 'X'
bpy.data.objects['Cube.001'].modifiers.new(name='B', type='DISPLACE')
bpy.data.objects['Cube.001'].modifiers['B'].vertex_group = 'G.R'
bpy.data.objects['Cube.001'].modifiers['B'].direction = 'Y'
