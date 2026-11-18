# WP 2.9: mesh.flip_normals reverses the selected faces keeping their first corner
# ([7,3,1,5] -> [7,5,1,3]); UVs stay on their corner; the selection and its history are kept.
# Then a vertex-mode selection that covers two faces, and a call with no face selected.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[5])
bpy.ops.mesh.flip_normals()
bpy.data.meshes['Cube'].polygons[5].vertices[1]
bpy.ops.mesh.select_mode(type='VERT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 1, 2, 3, 6, 7])
bpy.ops.mesh.flip_normals()
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.flip_normals()
bpy.ops.frap.select_elements(type='VERT', indices=[4])
bpy.ops.mesh.flip_normals(only_clnors=True)
