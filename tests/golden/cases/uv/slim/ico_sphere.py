# WP 8.1: SLIM on an ico sphere cut along the seams of its default UV map: the boundary starts
# on a circle, the interior at mean value coordinates, then 20 iterations of the symmetric
# Dirichlet energy with the flip-avoiding line search. (A UV sphere is not used: Blender's
# own result changes between runs with its run-dependent element order, D-19, since the
# circle starts at the first boundary edge.)
# golden: startup=empty compare=objects,context,mesh:Icosphere tier=T2 tol=1e-4
import bpy
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=2)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.seams_from_islands()
bpy.ops.uv.unwrap(method='MINIMUM_STRETCH', iterations=20)
bpy.ops.object.mode_set(mode='OBJECT')
