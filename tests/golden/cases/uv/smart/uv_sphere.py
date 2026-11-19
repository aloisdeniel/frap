# WP 3.8: uv.smart_project on UV spheres. frap's primitive_uv_sphere_add face order differs
# from Blender's (D-19) and the projection's area sort breaks ties by face order, so the
# layout differs from Blender's (island shapes, bbox height 0.672 vs 0.729 at 12x6): T3
# invariants (UVs in the unit square, islands disjoint).
# golden: startup=empty compare=objects,context,mesh:Sphere,mesh:Sphere.001 tier=T3 check=uv01,uv_no_overlap
import bpy
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6)
bpy.ops.mesh.primitive_uv_sphere_add(location=(3, 0, 0))
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.smart_project(island_margin=0.01)
bpy.ops.object.mode_set(mode='OBJECT')
