# WP 8.3.7: `object.vertex_group_mirror(use_topology=True)` on Suzanne (`ED_mesh_mirrtopo_init`:
# vertices paired by their edge-count hash), in Object Mode and in Edit Mode (the edit BMesh's
# own table), after vertices were moved off their mirrored positions, where the spatial mirror
# fails for them. Weights are read through DISPLACE modifiers (Side.L along X, Side.R along Y,
# Mid along Z) on the two kept stages.
# golden: startup=empty tier=T1 compare=objects,evaluated:Suzanne,evaluated:Suzanne.001
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.context.object.vertex_groups.new(name='Side.L')
bpy.context.object.vertex_groups.new(name='Side.R')
bpy.context.object.vertex_groups.new(name='Mid')
bpy.context.object.vertex_groups['Side.L'].add([0, 2, 4, 6, 8, 10, 46, 47, 100, 101, 102, 103, 200, 201, 202, 300, 301, 400, 401, 402], 0.75, 'REPLACE')
bpy.context.object.vertex_groups['Side.R'].add([1, 3, 5, 7, 9, 11, 48, 49, 150, 151, 152, 250, 251, 350, 351, 450, 451, 506], 0.25, 'REPLACE')
bpy.context.object.vertex_groups['Mid'].add([0, 1, 2, 3, 12, 13, 14, 15, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 499], 0.5, 'REPLACE')
bpy.data.meshes['Suzanne'].vertices[0].co = (0.5, 0.3, 0.2)
bpy.data.meshes['Suzanne'].vertices[3].co = (-0.2, 0.1, 0.0)
bpy.ops.object.vertex_group_set_active(group='Side.L')
bpy.ops.object.vertex_group_mirror(use_topology=True)
bpy.ops.object.vertex_group_set_active(group='Mid')
bpy.ops.object.vertex_group_mirror(use_topology=True, all_groups=True, flip_group_names=False)
bpy.ops.object.duplicate()
bpy.context.view_layer.objects.active = bpy.data.objects['Suzanne']
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Suzanne'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 1, 2, 5, 46, 100, 151, 200, 250, 300, 450, 499])
bpy.ops.object.vertex_group_set_active(group='Side.R')
bpy.ops.object.vertex_group_mirror(use_topology=True, all_groups=True)
bpy.ops.object.vertex_group_mirror(use_topology=False)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Suzanne'].modifiers.new(name='A', type='DISPLACE')
bpy.data.objects['Suzanne'].modifiers['A'].vertex_group = 'Side.L'
bpy.data.objects['Suzanne'].modifiers['A'].direction = 'X'
bpy.data.objects['Suzanne'].modifiers.new(name='B', type='DISPLACE')
bpy.data.objects['Suzanne'].modifiers['B'].vertex_group = 'Side.R'
bpy.data.objects['Suzanne'].modifiers['B'].direction = 'Y'
bpy.data.objects['Suzanne'].modifiers.new(name='C', type='DISPLACE')
bpy.data.objects['Suzanne'].modifiers['C'].vertex_group = 'Mid'
bpy.data.objects['Suzanne'].modifiers['C'].direction = 'Z'
bpy.data.objects['Suzanne.001'].modifiers.new(name='A', type='DISPLACE')
bpy.data.objects['Suzanne.001'].modifiers['A'].vertex_group = 'Side.L'
bpy.data.objects['Suzanne.001'].modifiers['A'].direction = 'X'
bpy.data.objects['Suzanne.001'].modifiers.new(name='B', type='DISPLACE')
bpy.data.objects['Suzanne.001'].modifiers['B'].vertex_group = 'Side.R'
bpy.data.objects['Suzanne.001'].modifiers['B'].direction = 'Y'
bpy.data.objects['Suzanne.001'].modifiers.new(name='C', type='DISPLACE')
bpy.data.objects['Suzanne.001'].modifiers['C'].vertex_group = 'Mid'
bpy.data.objects['Suzanne.001'].modifiers['C'].direction = 'Z'
