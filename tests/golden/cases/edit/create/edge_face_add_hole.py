# WP 2.8: mesh.edge_face_add closing the hole of an open box: the new quad takes its winding
# and attributes from the neighbouring faces (face_attribute_fill); then a single selected
# boundary vertex extends the selection along the hole and fills a triangle, leaving only the
# next boundary edge selected (USE_FACE_CREATE_SEL_EXTEND), and pressing again walks on.
# golden: startup=empty compare=objects,context,mesh:Box,mesh:Box2 tier=T2
import bpy
bpy.data.meshes.new('Box').from_pydata([(-1, -1, -1), (1, -1, -1), (1, 1, -1), (-1, 1, -1), (-1, -1, 1), (1, -1, 1), (1, 1, 1), (-1, 1, 1)], [], [(0, 3, 2, 1), (0, 1, 5, 4), (1, 2, 6, 5), (2, 3, 7, 6), (3, 0, 4, 7)])
bpy.data.objects.new('Box', bpy.data.meshes['Box'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Box'])
bpy.context.view_layer.objects.active = bpy.data.objects['Box']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[4, 5, 6, 7])
bpy.ops.mesh.edge_face_add()
bpy.data.meshes['Box'].total_face_sel
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes.new('Box2').from_pydata([(-1, -1, -1), (1, -1, -1), (1, 1, -1), (-1, 1, -1), (-1, -1, 1), (1, -1, 1), (1, 1, 1), (-1, 1, 1), (0, 0, 1.5)], [], [(0, 3, 2, 1), (0, 1, 5, 4), (1, 2, 6, 5), (2, 3, 7, 6), (3, 0, 4, 7)])
bpy.data.objects.new('Box2', bpy.data.meshes['Box2'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Box2'])
bpy.context.view_layer.objects.active = bpy.data.objects['Box2']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[5])
bpy.ops.mesh.edge_face_add()
bpy.ops.mesh.edge_face_add()
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[4, 5, 8])
bpy.ops.mesh.edge_face_add()
