# WP 8.1: SLIM on a cylinder with triangle-fan caps, one iteration, with no_flip (SLIM's
# reflection_mode) and fill_holes=False (ignored: SLIM always fills holes).
# golden: startup=empty compare=objects,context,mesh:Cylinder tier=T2 tol=1e-4
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=12, depth=3.0, end_fill_type='TRIFAN')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.seams_from_islands()
bpy.ops.uv.unwrap(method='MINIMUM_STRETCH', iterations=1, no_flip=True, fill_holes=False)
bpy.ops.object.mode_set(mode='OBJECT')
