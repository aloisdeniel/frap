# WP 4.6: Auto Merge with split, edge x edge. A second, smaller plane added in Edit Mode is
# rotated and moved so its edges cross the first plane's edges: every crossing cuts both
# edges and welds the cuts (BM_mesh_intersect_edges), and the faces the moved edges now lie
# on are split.
# golden: startup=factory compare=objects,context,mesh:Plane tier=T1 tol=1e-5
import bpy
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete()
bpy.ops.mesh.primitive_plane_add(size=2)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.primitive_plane_add(size=1, location=(0, 0, 0))
bpy.ops.transform.resize(value=(1.3, 0.6, 1))
bpy.ops.transform.rotate(value=0.4, orient_axis="Z")
bpy.ops.transform.translate(value=(0.9, 0.3, 0), use_automerge_and_split=True)
bpy.data.meshes['Plane'].total_vert_sel
bpy.data.meshes['Plane'].total_face_sel
bpy.ops.object.mode_set(mode='OBJECT')
