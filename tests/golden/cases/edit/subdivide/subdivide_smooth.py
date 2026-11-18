# WP 2.6: mesh.subdivide with smoothness (the dual-sphere interpolation over vertex normals,
# LINEAR falloff), on a cube and on an ico sphere; tolerance 1e-4 for the transcendental chain.
# golden: startup=factory compare=objects,mesh:Cube,mesh:Icosphere tier=T1 tol=1e-4
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.subdivide(number_cuts=2, smoothness=1.0)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=2, location=(3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.subdivide(smoothness=0.5)
