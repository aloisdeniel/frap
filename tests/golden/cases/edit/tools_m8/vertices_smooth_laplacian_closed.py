# WP 8.3.3: mesh.vertices_smooth_laplacian with lambda_factor > 1 on closed meshes (T2, tol
# 1e-5): a torus smoothed three times with lambda 2.5 (volume preserved), an ico sphere with
# lambda 3 and a jittered cube grid (subdivided cube) with lambda 1.8 and a border weight.
# Ill-conditioned runs (many repeats with a large lambda, which grow without bound) are not
# T2-comparable: the solves amplify the 1e-7 differences of another LU (faer instead of
# Eigen's SparseLU) and of float summation order; they stay out of the goldens.
# golden: startup=factory compare=objects,context,mesh:Torus,mesh:Icosphere,mesh:Cube tier=T2 tol=1e-5 order=exact
import bpy
bpy.ops.object.delete()
bpy.ops.mesh.primitive_torus_add(location=(0, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.vertices_smooth_laplacian(repeat=3, lambda_factor=2.5, lambda_border=1.0)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=3, location=(4, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.vertices_smooth_laplacian(repeat=2, lambda_factor=3.0)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_cube_add(location=(0, 4, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.subdivide(number_cuts=3)
bpy.ops.transform.vertex_random(offset=0.1, seed=5)
bpy.ops.mesh.vertices_smooth_laplacian(repeat=2, lambda_factor=1.8, lambda_border=0.5)
