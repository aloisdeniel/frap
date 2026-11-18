# WP 2.8: mesh.edge_face_add on a net of wire edges (a 3x3 lattice with a diagonal): the
# edge-net fill finds the shortest closing paths and makes one face per cell; then an open
# chain of edges is closed by edgenet_prepare and filled.
# golden: startup=empty compare=objects,context,mesh:Net,mesh:Open tier=T1
import bpy
bpy.data.meshes.new('Net').from_pydata([(0, 0, 0), (1, 0, 0), (2, 0, 0), (0, 1, 0), (1, 1, 0), (2, 1, 0), (0, 2, 0), (1, 2, 0), (2, 2, 0)], [(0, 1), (1, 2), (3, 4), (4, 5), (6, 7), (7, 8), (0, 3), (3, 6), (1, 4), (4, 7), (2, 5), (5, 8), (0, 4)], [])
bpy.data.objects.new('Net', bpy.data.meshes['Net'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Net'])
bpy.context.view_layer.objects.active = bpy.data.objects['Net']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.edge_face_add()
bpy.data.meshes['Net'].total_face_sel
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.select_all(action='DESELECT')
bpy.data.meshes.new('Open').from_pydata([(0, 0, 0), (1, 0, 0), (1.5, 1, 0), (1, 2, 0), (0, 2, 0), (-0.5, 1, 0)], [(0, 1), (1, 2), (2, 3), (3, 4), (4, 5)], [])
bpy.data.objects.new('Open', bpy.data.meshes['Open'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Open'])
bpy.context.view_layer.objects.active = bpy.data.objects['Open']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.edge_face_add()
