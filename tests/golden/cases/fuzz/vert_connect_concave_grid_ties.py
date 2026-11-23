# M8 stabilization round 2 (fuzz seed 0xea09980f5e4881b3): `vert_connect_concave` of a grid
# dissolved into one n-gon (collinear corners): the beauty triangulation's diagonals have equal
# lengths, and `bm_face_split_by_concave` sorts them with libc++'s `std::sort` in creation
# order (`frap_bmesh::cxx_sort`), whatever slots the earlier dissolve freed.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Grid tier=T2 tol=0.0001
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, size=1.46)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.dissolve_faces()
bpy.ops.mesh.vert_connect_concave()
