# WP 6.3: the Edit Mode shape-key tools. `mesh.blend_from_shape` (add: by the block's offset
# from its relative key; blend: toward the block; the unset `shape` is the basis; a locked
# active block refuses) and `mesh.shape_propagate_to_all` (the selected vertices' positions go
# into every block), then the result in the blocks after leaving Edit Mode (linked duplicates
# showing single blocks), and the `ShapeKey.normals_*_get` functions.
# golden: startup=factory tier=T1 compare=mesh:Cube,evaluated:Cube,evaluated:Cube.001,evaluated:Cube.002
import bpy
bpy.ops.mesh.blend_from_shape()
bpy.ops.object.duplicate(linked=True)
bpy.ops.object.duplicate(linked=True)
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.blend_from_shape()
bpy.ops.mesh.shape_propagate_to_all()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Cube'].shape_key_add(name='Basis')
bpy.data.objects['Cube'].shape_key_add(name='A', from_mix=False)
bpy.data.objects['Cube'].shape_key_add(name='B', from_mix=False)
bpy.data.shape_keys['Key'].key_blocks['A'].data.foreach_set('co', [1.5, 1, 1, 1.5, 1, -1, 1.5, -1, 1, 1.5, -1, -1, -0.5, 1, 1, -0.5, 1, -1, -0.5, -1, 1, -0.5, -1, -1])
bpy.data.shape_keys['Key'].key_blocks['B'].data.foreach_set('co', [1.5, 2, 1, 1.5, 2, -1, 1.5, 0, 1, 1.5, 0, -1, -0.5, 2, 1, -0.5, 2, -1, -0.5, 0, 1, -0.5, 0, -1])
bpy.data.shape_keys['Key'].key_blocks['B'].relative_key = bpy.data.shape_keys['Key'].key_blocks['A']
bpy.data.shape_keys['Key'].key_blocks['A'].normals_vertex_get()
bpy.data.shape_keys['Key'].key_blocks['A'].normals_polygon_get()
bpy.data.shape_keys['Key'].key_blocks['A'].normals_split_get()
bpy.data.objects['Cube'].active_shape_key_index = 0
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.shape_propagate_to_all()
bpy.ops.mesh.blend_from_shape(shape='B')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 1, 2, 3])
bpy.ops.mesh.blend_from_shape(shape='B', blend=0.5, add=True)
bpy.ops.mesh.blend_from_shape(shape='Nope')
bpy.ops.frap.select_elements(type='VERT', indices=[4, 5])
bpy.ops.mesh.blend_from_shape(shape='A', blend=0.25, add=False)
bpy.ops.frap.select_elements(type='VERT', indices=[6])
bpy.ops.mesh.blend_from_shape(add=False)
bpy.ops.frap.select_elements(type='VERT', indices=[7])
bpy.ops.transform.translate(value=(0, 0, 3))
bpy.ops.mesh.shape_propagate_to_all()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.shape_keys['Key'].key_blocks['A'].lock_shape = True
bpy.data.objects['Cube'].active_shape_key_index = 1
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.blend_from_shape(shape='B')
bpy.ops.mesh.shape_propagate_to_all()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Cube.001'].show_only_shape_key = True
bpy.data.objects['Cube.001'].active_shape_key_index = 1
bpy.data.objects['Cube.002'].show_only_shape_key = True
bpy.data.objects['Cube.002'].active_shape_key_index = 2
