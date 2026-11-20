# WP 6.3: shape keys in Edit Mode (`BM_mesh_bm_from_me` / `bm_to_mesh_shape`). Entering takes
# the vertex positions from the active block; leaving writes them into it, and when the edited
# block is the basis of others (directly or through a chain) they are offset by the edit
# ("edit-mode basis propagation"); editing a non-basis block leaves the mesh positions at the
# reference key's. Changing `active_shape_key_index` in Edit Mode reloads the edit mesh. The
# linked duplicates show single blocks (`show_only_shape_key`).
# golden: startup=factory tier=T1 compare=mesh:Cube,evaluated:Cube,evaluated:Cube.001,evaluated:Cube.002,evaluated:Cube.003
import bpy
bpy.ops.object.duplicate(linked=True)
bpy.ops.object.duplicate(linked=True)
bpy.ops.object.duplicate(linked=True)
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.data.objects['Cube'].shape_key_add(name='Basis')
bpy.data.objects['Cube'].shape_key_add(name='A', from_mix=False)
bpy.data.objects['Cube'].shape_key_add(name='B', from_mix=False)
bpy.data.objects['Cube'].shape_key_add(name='Free', from_mix=False)
bpy.data.shape_keys['Key'].key_blocks['A'].data.foreach_set('co', [1.5, 1, 1, 1.5, 1, -1, 1.5, -1, 1, 1.5, -1, -1, -0.5, 1, 1, -0.5, 1, -1, -0.5, -1, 1, -0.5, -1, -1])
bpy.data.shape_keys['Key'].key_blocks['B'].data.foreach_set('co', [1.5, 2, 1, 1.5, 2, -1, 1.5, 0, 1, 1.5, 0, -1, -0.5, 1, 1, -0.5, 1, -1, -0.5, -1, 1, -0.5, -1, -1])
bpy.data.shape_keys['Key'].key_blocks['B'].relative_key = bpy.data.shape_keys['Key'].key_blocks['A']
bpy.data.shape_keys['Key'].key_blocks['Free'].relative_key = bpy.data.shape_keys['Key'].key_blocks['Free']
bpy.data.shape_keys['Key'].key_blocks['A'].value = 1.0
bpy.data.shape_keys['Key'].key_blocks['B'].value = 0.5
bpy.data.objects['Cube'].active_shape_key_index = 0
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.transform.translate(value=(0, 0, 1))
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Cube'].active_shape_key_index = 1
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.transform.translate(value=(0, 2, 0))
bpy.data.objects['Cube'].active_shape_key_index = 3
bpy.ops.transform.translate(value=(-3, 0, 0))
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Cube.001'].show_only_shape_key = True
bpy.data.objects['Cube.001'].active_shape_key_index = 1
bpy.data.objects['Cube.002'].show_only_shape_key = True
bpy.data.objects['Cube.002'].active_shape_key_index = 2
bpy.data.objects['Cube.003'].show_only_shape_key = True
bpy.data.objects['Cube.003'].active_shape_key_index = 3
