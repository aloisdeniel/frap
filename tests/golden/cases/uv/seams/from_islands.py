# WP 3.8: uv.seams_from_islands marks the island boundaries of the UV map: after
# uv.smart_project every cube edge; mark_sharp also clears their smoothness. With the factory
# cross layout of the cube, only the 7 cut edges; nothing to mark: CANCELLED.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.seams_from_islands()
bpy.ops.mesh.mark_seam(clear=True)
bpy.ops.uv.smart_project()
bpy.ops.uv.seams_from_islands(mark_sharp=True)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.seams_from_islands()
bpy.ops.object.mode_set(mode='OBJECT')
