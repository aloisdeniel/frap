# WP 8.1: uv.unwrap(method='ANGLE_BASED') runs ABF++ (p_chart_abf_solve) before LSCM. A UV
# sphere cut along the seams of its default UV map (uv.seams_from_islands) is not developable,
# so ABF++ and plain LSCM give different layouts; the result matches Blender's within 1e-5.
# golden: startup=empty compare=objects,context,mesh:Sphere tier=T2 tol=1e-5
import bpy
bpy.ops.mesh.primitive_uv_sphere_add(segments=16, ring_count=8)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.seams_from_islands()
bpy.ops.uv.unwrap(method='ANGLE_BASED')
bpy.ops.object.mode_set(mode='OBJECT')
