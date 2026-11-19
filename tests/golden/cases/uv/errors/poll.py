# WP 3.8: the UV operators need a mesh in Edit Mode (ED_operator_uvmap); the UV-editing ones
# (pack_islands, average_islands_scale, mark_seam, seams_from_islands, pin) also need a UV map
# (ED_operator_uvedit); nothing to unwrap (no face visible / selected): CANCELLED.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T0
import bpy
bpy.ops.uv.unwrap()
bpy.ops.uv.smart_project()
bpy.ops.uv.pack_islands()
bpy.ops.uv.cube_project()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.uv_texture_remove()
bpy.ops.uv.pack_islands()
bpy.ops.uv.average_islands_scale()
bpy.ops.uv.pin()
bpy.ops.uv.mark_seam()
bpy.ops.uv.seams_from_islands()
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.uv.unwrap()
bpy.ops.uv.cube_project()
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.pack_islands()
bpy.ops.uv.unwrap(method='MINIMUM_STRETCH')
bpy.ops.uv.pack_islands(rotate_method='DIAGONAL')
bpy.ops.mesh.delete(type='VERT')
bpy.ops.uv.unwrap()
bpy.ops.uv.reset()
