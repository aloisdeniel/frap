# M3+M4 stabilization: uv.smart_project (defaults) on the default cube after applying a
# level-1 SUBSURF. frap's SUBSURF positions are within 1.2e-7 of Blender's (T2), and the 24
# faces of the subdivided cube have equal areas in exact arithmetic, so the projection's area
# sort orders the ties differently and the layout differs (islands, total UV area 0.529 vs
# 0.427; deviations.md §2): T3 invariants (islands disjoint, topology kept; with the default
# island_margin 0 Blender itself puts a few UVs a rounding step outside [0, 1]).
# golden: startup=factory compare=objects,mesh:Cube tier=T3 check=uv_no_overlap,faces:24-24,euler:2
import bpy
bpy.ops.object.modifier_add(type='SUBSURF')
bpy.ops.object.modifier_apply(modifier='Subdivision')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.smart_project()
bpy.ops.object.mode_set(mode='OBJECT')
