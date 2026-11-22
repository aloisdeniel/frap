# WP 8.3.7: `object.vertex_group_smooth` in Edit Mode: Jacobi passes over the selected vertices with
# visible neighbours (`factor`, `repeat`), `expand` towards larger (> 0) or smaller (< 0)
# neighbours, the subsets (`ACTIVE`, `ALL`) without the locked groups (`vgroup_validmap` minus
# `lock_weight`), zero weights removed afterwards on every vertex, a hidden vertex not read, two
# meshes edited together, and `No meshes with vertex groups found` (CANCELLED). Weights are read
# through DISPLACE modifiers (A along X, B along Y, C along Z).
# golden: startup=empty tier=T1 compare=objects,evaluated:Grid,evaluated:Grid.001
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=5, y_subdivisions=4, size=2)
bpy.ops.mesh.primitive_grid_add(x_subdivisions=3, y_subdivisions=3, size=2, location=(3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.object.vertex_group_smooth()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Grid'].vertex_groups.new(name='A')
bpy.data.objects['Grid'].vertex_groups.new(name='B')
bpy.data.objects['Grid'].vertex_groups.new(name='C')
bpy.data.objects['Grid'].vertex_groups['A'].add([0, 1, 2, 7], 1.0, 'REPLACE')
bpy.data.objects['Grid'].vertex_groups['A'].add([12], 0.0, 'REPLACE')
bpy.data.objects['Grid'].vertex_groups['B'].add([4, 8, 9, 13, 14, 18, 19], 0.6, 'REPLACE')
bpy.data.objects['Grid'].vertex_groups['B'].add([10, 11], 0.2, 'REPLACE')
bpy.data.objects['Grid'].vertex_groups['C'].add([5, 6, 7, 8], 0.9, 'REPLACE')
bpy.data.objects['Grid'].vertex_groups['C'].lock_weight = True
bpy.data.objects['Grid.001'].vertex_groups.new(name='A')
bpy.data.objects['Grid.001'].vertex_groups['A'].add([4], 1.0, 'REPLACE')
bpy.ops.object.vertex_group_smooth()
bpy.data.objects['Grid'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Grid']
bpy.ops.object.vertex_group_set_active(group='A')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[6])
bpy.ops.mesh.hide(unselected=False)
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.object.vertex_group_smooth(factor=0.5, repeat=3, group_select_mode='ALL')
bpy.ops.object.vertex_group_smooth(factor=0.8, expand=0.5)
bpy.ops.mesh.reveal(select=False)
bpy.ops.object.vertex_group_set_active(group='B')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[3, 4, 9, 10, 15, 16, 17])
bpy.ops.object.vertex_group_smooth(factor=1.0, repeat=2, expand=-0.3)
bpy.ops.object.vertex_group_set_active(group='C')
bpy.ops.object.vertex_group_smooth()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Grid'].vertex_groups['A'].weight(12)
bpy.data.objects['Camera'].location = (bpy.data.objects['Grid'].vertex_groups['C'].weight(5), bpy.data.objects['Grid'].vertex_groups['A'].weight(6), bpy.data.objects['Grid.001'].vertex_groups['A'].weight(0))
bpy.data.objects['Grid'].modifiers.new(name='A', type='DISPLACE')
bpy.data.objects['Grid'].modifiers['A'].vertex_group = 'A'
bpy.data.objects['Grid'].modifiers['A'].direction = 'X'
bpy.data.objects['Grid'].modifiers.new(name='B', type='DISPLACE')
bpy.data.objects['Grid'].modifiers['B'].vertex_group = 'B'
bpy.data.objects['Grid'].modifiers['B'].direction = 'Y'
bpy.data.objects['Grid'].modifiers.new(name='C', type='DISPLACE')
bpy.data.objects['Grid'].modifiers['C'].vertex_group = 'C'
bpy.data.objects['Grid'].modifiers['C'].direction = 'Z'
bpy.data.objects['Grid.001'].modifiers.new(name='A', type='DISPLACE')
bpy.data.objects['Grid.001'].modifiers['A'].vertex_group = 'A'
bpy.data.objects['Grid.001'].modifiers['A'].direction = 'Z'
