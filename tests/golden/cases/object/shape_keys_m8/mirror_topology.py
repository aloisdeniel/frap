# WP 8.3.7: `object.shape_key_mirror` on Suzanne, spatially and with `use_topology=True`
# (`ED_mesh_mirrtopo_init`): after vertices of the mesh were moved off their mirrored positions,
# the spatial mirror fails for them (WARNING) while the topology mirror still pairs them. The
# blocks are compared through linked duplicates with `show_only_shape_key`.
# golden: startup=empty tier=T1 compare=objects,evaluated:Suzanne.001,evaluated:Suzanne.002,evaluated:Suzanne.003
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.ops.object.shape_key_add(from_mix=False)
bpy.ops.object.shape_key_add(from_mix=False)
bpy.ops.object.shape_key_add(from_mix=False)
bpy.ops.object.shape_key_add(from_mix=False)
bpy.data.shape_keys['Key'].key_blocks['Key 1'].data[0].co = (0.6, 0.9, 0.1)
bpy.data.shape_keys['Key'].key_blocks['Key 1'].data[3].co = (-0.4, 0.2, -0.3)
bpy.data.shape_keys['Key'].key_blocks['Key 1'].data[100].co = (0.0, 0.0, 2.0)
bpy.data.shape_keys['Key'].key_blocks['Key 1'].data[499].co = (0.25, -1.0, 0.5)
bpy.data.shape_keys['Key'].key_blocks['Key 2'].data[1].co = (-0.6, 0.9, 0.1)
bpy.data.shape_keys['Key'].key_blocks['Key 2'].data[200].co = (0.3, 0.3, 0.3)
bpy.data.shape_keys['Key'].key_blocks['Key 3'].data[5].co = (1.0, 1.0, 1.0)
bpy.data.objects['Suzanne'].active_shape_key_index = 1
bpy.ops.object.shape_key_mirror()
bpy.data.meshes['Suzanne'].vertices[0].co = (0.5, 0.3, 0.2)
bpy.data.meshes['Suzanne'].vertices[200].co = (-0.2, 0.1, 0.0)
bpy.data.objects['Suzanne'].active_shape_key_index = 2
bpy.ops.object.shape_key_mirror(use_topology=True)
bpy.data.objects['Suzanne'].active_shape_key_index = 3
bpy.ops.object.shape_key_mirror()
bpy.ops.object.duplicate(linked=True)
bpy.data.objects['Suzanne.001'].show_only_shape_key = True
bpy.data.objects['Suzanne.001'].active_shape_key_index = 1
bpy.ops.object.duplicate(linked=True)
bpy.data.objects['Suzanne.002'].show_only_shape_key = True
bpy.data.objects['Suzanne.002'].active_shape_key_index = 2
bpy.ops.object.duplicate(linked=True)
bpy.data.objects['Suzanne.003'].show_only_shape_key = True
bpy.data.objects['Suzanne.003'].active_shape_key_index = 3
