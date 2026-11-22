# WP 8.1: SLIM on Suzanne (narrowed, as uv/abf/suzanne_seams.py) cut along the seams of her
# default UV map: five islands, each with its own SLIM system, then averaged and packed.
# golden: startup=empty compare=objects,context,mesh:Suzanne tier=T2 tol=1e-4
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.transform.resize(value=(0.6, 1.0, 1.0))
bpy.ops.uv.seams_from_islands()
bpy.ops.uv.unwrap(method='MINIMUM_STRETCH')
bpy.ops.object.mode_set(mode='OBJECT')
