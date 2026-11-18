# WP 4.6: Auto Merge with split (`use_automerge_and_split=True`, EDBM_automerge_and_split).
# A corner of the plane is extruded into a wire edge whose new vertex is dropped on the
# middle of the opposite edge: that edge is cut there, the vertex welds into the cut, and the
# face is split along the wire edge (BM_mesh_intersect_edges with split_faces).
# golden: startup=factory compare=objects,context,mesh:Plane tier=T1 tol=1e-5
import bpy
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete()
bpy.ops.mesh.primitive_plane_add(size=2)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0])
bpy.ops.mesh.extrude_vertices_move(TRANSFORM_OT_translate={"value": (1, 2, 0), "use_automerge_and_split": True})
bpy.data.meshes['Plane'].total_vert_sel
bpy.data.meshes['Plane'].total_edge_sel
