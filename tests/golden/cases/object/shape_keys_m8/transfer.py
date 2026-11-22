# WP 8.3.7: `object.shape_key_transfer` (the Python operator `ShapeTransfer`): the active block of
# the other selected mesh becomes a new block of the active mesh (a `Basis` first, value 0,
# active, `show_only_shape_key` on) with the offsets (`OFFSET`), the corner triangles
# (`RELATIVE_FACE`) or the edges and vertex normals (`RELATIVE_EDGE`) of the source's
# deformation, `use_clamp` keeping each vertex's offset length; the errors (no or several other
# selected objects, a non-mesh, a source without shape keys) and the WARNING for another vertex
# count. The target shows its last block (`evaluated:Target`); the other blocks through linked
# duplicates.
# golden: startup=factory tier=T1 compare=objects,evaluated:Cube,evaluated:Target,evaluated:Target.001,evaluated:Target.002,evaluated:Target.003
import bpy
bpy.ops.object.shape_key_transfer()
bpy.ops.object.shape_key_add(from_mix=False)
bpy.ops.object.shape_key_add(from_mix=False)
bpy.data.shape_keys['Key'].key_blocks['Key 1'].name = 'Smile'
bpy.data.shape_keys['Key'].key_blocks['Smile'].data.foreach_set('co', [1.5, 1, 1.25, 1, 1, -1, 1.25, -1.25, 1, 1, -1, -1, -1, 1, 1.75, -1, 1, -1, -1, -1, 1, -0.5, -1.5, -1])
bpy.ops.mesh.primitive_cube_add(size=1, location=(3, 0, 0))
bpy.context.object.name = 'Target'
bpy.data.meshes['Cube.001'].vertices.foreach_set('co', [0.5, 0.5, 1.0, 0.75, 0.5, -0.5, 0.5, -0.5, 0.5, 0.5, -0.5, -0.5, -0.5, 0.5, 0.5, -0.5, 0.5, -0.5, -0.25, -0.5, 0.5, -0.5, -0.5, -0.5])
bpy.ops.object.shape_key_transfer()
bpy.data.objects['Cube'].select_set(True)
bpy.data.objects['Camera'].select_set(True)
bpy.ops.object.shape_key_transfer()
bpy.data.objects['Cube'].select_set(False)
bpy.ops.object.shape_key_transfer()
bpy.data.objects['Camera'].select_set(False)
bpy.data.objects['Cube'].select_set(True)
bpy.data.objects['Cube'].active_shape_key_index = 1
bpy.ops.object.shape_key_transfer()
bpy.data.objects['Camera'].location = (bpy.data.objects['Target'].active_shape_key_index, bpy.data.objects['Target'].show_only_shape_key, bpy.data.shape_keys['Key.001'].key_blocks['Smile'].value)
bpy.data.objects['Camera'].scale = (bpy.data.shape_keys['Key.001'].key_blocks['Basis'].value, bpy.data.shape_keys['Key.001'].key_blocks['Smile'].slider_max, 1)
bpy.ops.object.shape_key_transfer(mode='RELATIVE_FACE')
bpy.ops.object.shape_key_transfer(mode='RELATIVE_EDGE')
bpy.ops.object.shape_key_transfer(mode='RELATIVE_FACE', use_clamp=True)
bpy.ops.object.shape_key_transfer(mode='OFFSET', use_clamp=True)
bpy.data.objects['Light'].location = (bpy.data.objects['Target'].active_shape_key_index, bpy.data.shape_keys['Key.001'].key_blocks['Smile.004'].value, 0)
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=4, location=(0, 4, 0))
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.shape_key_transfer()
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.shape_key_transfer()
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.data.objects['Target'].select_set(True)
bpy.data.objects['Target'].active_shape_key_index = 1
bpy.ops.object.shape_key_transfer(mode='RELATIVE_EDGE', use_clamp=True)
bpy.context.view_layer.objects.active = bpy.data.objects['Target']
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Target'].select_set(True)
bpy.data.objects['Target'].active_shape_key_index = 2
bpy.ops.object.duplicate(linked=True)
bpy.data.objects['Target.001'].active_shape_key_index = 3
bpy.ops.object.duplicate(linked=True)
bpy.data.objects['Target.002'].active_shape_key_index = 4
bpy.ops.object.duplicate(linked=True)
bpy.data.objects['Target.003'].active_shape_key_index = 1
bpy.data.objects['Cube'].show_only_shape_key = True
bpy.data.objects['Cube'].active_shape_key_index = 2
