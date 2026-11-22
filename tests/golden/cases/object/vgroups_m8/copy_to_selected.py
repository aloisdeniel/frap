# WP 8.3.7: `object.vertex_group_copy_to_selected`: the selected meshes with the active mesh's
# vertex count take its groups (names, locks, active index) and weights, also when they had no
# weights yet; a linked duplicate (same mesh) counts as done; two meshes without weights count as
# done without copying; nothing done or a vertex count that differs is the `Copy vertex groups to
# selected: N done, M failed (…)` error. (A failing call changes nothing in frap, D-148: the case
# only lets calls fail where Blender changes nothing either.) Group order, locks and the active
# index are read into object transforms; weights through DISPLACE modifiers.
# golden: startup=factory tier=T1 compare=objects,evaluated:Cube.001,evaluated:Cube.002,evaluated:Cube.003
import bpy
bpy.data.objects['Cube'].vertex_groups.new(name='A')
bpy.data.objects['Cube'].vertex_groups.new(name='B')
bpy.ops.object.vertex_group_copy_to_selected()
bpy.data.objects['Cube'].vertex_groups['A'].add([0, 1, 2], 0.5, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups['B'].add([2, 3, 4], 0.25, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups['B'].lock_weight = True
bpy.data.objects['Cube'].vertex_groups.active_index = 0
bpy.ops.mesh.primitive_cube_add(size=1, location=(3, 0, 0))
bpy.context.object.vertex_groups.new(name='Old')
bpy.context.object.vertex_groups.new(name='Older')
bpy.context.object.vertex_groups.new(name='Oldest')
bpy.ops.mesh.primitive_cube_add(size=1, location=(-3, 0, 0))
bpy.context.object.vertex_groups.new(name='Z')
bpy.context.object.vertex_groups['Z'].add([5, 6, 7], 1.0, 'REPLACE')
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=4, location=(0, 3, 0))
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Sphere'].select_set(True)
bpy.data.objects['Cube'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.vertex_group_copy_to_selected()
bpy.data.objects['Sphere'].vertex_groups.new(name='S')
bpy.data.objects['Sphere'].vertex_groups['S'].add([0], 1.0, 'REPLACE')
bpy.ops.object.vertex_group_copy_to_selected()
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube.001'].select_set(True)
bpy.data.objects['Cube.002'].select_set(True)
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.vertex_group_copy_to_selected()
bpy.data.objects['Camera'].location = (bpy.data.objects['Cube.001'].vertex_groups['A'].index, bpy.data.objects['Cube.001'].vertex_groups['B'].index, bpy.data.objects['Cube.001'].vertex_groups.active_index)
bpy.data.objects['Cube.001'].vertex_groups[2]
bpy.data.objects['Camera'].scale = (bpy.data.objects['Cube.002'].vertex_groups['B'].lock_weight, bpy.data.objects['Cube.002'].vertex_groups['A'].lock_weight, bpy.data.objects['Cube.002'].vertex_groups.active_index)
bpy.data.objects['Cube.002'].vertex_groups['Z']
bpy.data.objects['Cube.001'].vertex_groups['Old']
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.duplicate(linked=True)
bpy.data.objects['Cube'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.vertex_group_copy_to_selected()
bpy.ops.mesh.primitive_cube_add(size=1, location=(0, -3, 0))
bpy.context.object.vertex_groups.new(name='N')
bpy.ops.mesh.primitive_cube_add(size=1, location=(0, -5, 0))
bpy.context.object.vertex_groups.new(name='M')
bpy.data.objects['Cube.004'].select_set(True)
bpy.ops.object.vertex_group_copy_to_selected()
bpy.data.objects['Light'].location = (bpy.data.objects['Cube.004'].vertex_groups['N'].index, bpy.data.objects['Cube.004'].vertex_groups.active_index, 0)
bpy.data.objects['Cube.004'].vertex_groups[1]
bpy.data.objects['Cube.001'].modifiers.new(name='A', type='DISPLACE')
bpy.data.objects['Cube.001'].modifiers['A'].vertex_group = 'A'
bpy.data.objects['Cube.001'].modifiers['A'].direction = 'X'
bpy.data.objects['Cube.001'].modifiers.new(name='B', type='DISPLACE')
bpy.data.objects['Cube.001'].modifiers['B'].vertex_group = 'B'
bpy.data.objects['Cube.001'].modifiers['B'].direction = 'Y'
bpy.data.objects['Cube.002'].modifiers.new(name='A', type='DISPLACE')
bpy.data.objects['Cube.002'].modifiers['A'].vertex_group = 'A'
bpy.data.objects['Cube.002'].modifiers['A'].direction = 'X'
bpy.data.objects['Cube.002'].modifiers.new(name='B', type='DISPLACE')
bpy.data.objects['Cube.002'].modifiers['B'].vertex_group = 'B'
bpy.data.objects['Cube.002'].modifiers['B'].direction = 'Y'
bpy.data.objects['Cube.003'].modifiers.new(name='A', type='DISPLACE')
bpy.data.objects['Cube.003'].modifiers['A'].vertex_group = 'A'
bpy.data.objects['Cube.003'].modifiers['A'].direction = 'X'
