# WP 6.3: edit-mode basis propagation once vertices are created while editing. Subdivide
# creates vertices from an example vertex (`BM_vert_create` sets their `CD_SHAPE_KEYINDEX` to
# `ORIGINDEX_NONE`), so `bm_to_mesh_shape` drops the offset propagation altogether: editing the
# basis moves the basis only, K1 keeps its (interpolated) shape. Duplicating copies the key
# index (`bmo_dupe`), so the propagation still applies in the second round.
# golden: startup=factory tier=T1 compare=mesh:Cube,evaluated:Cube.001,evaluated:Cube.002
import bpy
bpy.ops.object.duplicate(linked=True)
bpy.ops.object.duplicate(linked=True)
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.data.objects['Cube'].shape_key_add(name='Basis')
bpy.data.objects['Cube'].shape_key_add(name='K1', from_mix=False)
bpy.data.shape_keys['Key'].key_blocks['K1'].data.foreach_set('co', [1.5, 1, 1, 1.5, 1, -1, 1.5, -1, 1, 1.5, -1, -1, -0.5, 1, 1, -0.5, 1, -1, -0.5, -1, 1, -0.5, -1, -1])
bpy.data.objects['Cube'].active_shape_key_index = 0
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.subdivide()
bpy.ops.transform.translate(value=(0, 0, 1))
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 1])
bpy.ops.mesh.duplicate()
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.transform.translate(value=(0, 1, 0))
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Cube.001'].show_only_shape_key = True
bpy.data.objects['Cube.001'].active_shape_key_index = 0
bpy.data.objects['Cube.002'].show_only_shape_key = True
bpy.data.objects['Cube.002'].active_shape_key_index = 1
