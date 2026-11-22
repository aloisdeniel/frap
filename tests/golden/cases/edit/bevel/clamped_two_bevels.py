# WP 2.11: a vertex bevel clamped at the edge midpoints leaves zero-length edges; beveling it
# again snaps face corners onto them for the UV interpolation (closest_to_ray_v3's
# zero-direction guard: finite UVs, no NaN). The first bevel frees its rebuilt faces in the
# iteration order of Blender's pointer set (WP 8.2, `bevel/ptrset.rs`), so the second one is
# index-exact. The same bug showed on Suzanne (vertex bevel 0.05 clamped, then WIDTH 0.01).
# golden: startup=factory compare=mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bevel(offset=5, affect='VERTICES', clamp_overlap=True, segments=2)
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bevel(offset_type='WIDTH', offset=0.1, segments=2)
