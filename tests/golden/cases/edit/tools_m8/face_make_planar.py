# WP 8.3.3: mesh.face_make_planar (iterative planarisation, T2): a cube with two corners
# raised, a randomised grid with every face, part of the faces with a factor and repeats,
# triangles left alone, a non-planar n-gon; nothing selected: CANCELLED.
# golden: startup=factory compare=objects,context,mesh:Grid,mesh:Cube tier=T2 tol=1e-5 order=exact
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 2])
bpy.ops.transform.translate(value=(0.0, 0.0, 0.4))
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1, 2, 3, 4, 5])
bpy.ops.mesh.face_make_planar()
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.face_make_planar()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=6, size=2, location=(4, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.transform.vertex_random(offset=0.3, seed=5)
bpy.ops.mesh.face_make_planar(repeat=5)
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1, 2, 7, 8, 9])
bpy.ops.mesh.quads_convert_to_tris()
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[10, 11, 12, 13, 14, 20, 21, 22])
bpy.ops.mesh.face_make_planar(factor=0.6, repeat=30)
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[15, 16, 17])
bpy.ops.mesh.dissolve_faces()
bpy.ops.mesh.face_make_planar(repeat=100)
