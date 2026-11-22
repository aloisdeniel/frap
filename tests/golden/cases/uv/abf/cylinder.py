# WP 8.1: ABF++ on a cylinder whose caps are triangle fans (interior centre vertices: the
# Newton iterations run), cut along the seams of its default UV map; three islands packed.
# golden: startup=empty compare=objects,context,mesh:Cylinder tier=T2 tol=1e-5
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=12, depth=3.0, end_fill_type='TRIFAN')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.seams_from_islands()
bpy.ops.uv.unwrap(method='ANGLE_BASED', margin=0.02)
bpy.ops.object.mode_set(mode='OBJECT')
