# WP 3.8: a closed cube without seams cannot be flattened: uv.unwrap still finishes (WARNING
# "Unwrap failed to solve 1 of 1 island(s), …") and only repacks the reset UVs; a mesh without
# a UV map gets one ("UVMap") first.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cube.001 tier=T2 tol=1e-4
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.unwrap(method='CONFORMAL')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0))
bpy.ops.mesh.uv_texture_remove()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.unwrap(method='CONFORMAL')
bpy.ops.object.mode_set(mode='OBJECT')
