# WP 8.3.1: mesh.select_interior_faces: a cube bisected at z=0 whose cut loop is filled with
# a face (every edge of that face has three faces) is selected as interior; a second fill makes
# two face groups compete; a mesh without non-manifold edges keeps its selection.
# golden: startup=empty compare=mesh:Cube tier=T1
import bpy
bpy.ops.mesh.primitive_cube_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_interior_faces()
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bisect(plane_co=(0, 0, 0), plane_no=(0, 0, 1))
bpy.ops.mesh.edge_face_add()
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bisect(plane_co=(0, 0, 0.5), plane_no=(0, 0, 1))
bpy.ops.mesh.edge_face_add()
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_interior_faces()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.select_poly'].name = 'two_fills'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[0])
bpy.ops.mesh.select_interior_faces()
