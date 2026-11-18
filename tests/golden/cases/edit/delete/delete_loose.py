# WP 2.7: mesh.delete_loose removes selected wire edges and loose vertices (and, with
# use_faces, selected faces whose edges are all boundary edges); each kind only when some
# edit mesh has that kind selected. A plane joined into the cube loses its face first.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.primitive_plane_add(size=1, location=(3, 0, 0))
bpy.ops.mesh.delete(type='ONLY_FACE')
bpy.ops.mesh.primitive_plane_add(size=1, location=(0, 3, 0))
bpy.ops.mesh.primitive_plane_add(size=1, location=(0, -3, 0))
bpy.ops.frap.select_elements(type='EDGE', indices=[12, 13])
bpy.ops.mesh.delete_loose()
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.delete_loose(use_verts=False, use_edges=False)
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.delete_loose(use_faces=True)
