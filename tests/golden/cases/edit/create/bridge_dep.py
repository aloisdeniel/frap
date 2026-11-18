# WP 2.8 (needs 2.7 weld_verts, 2.10 triangulate): bridge_edge_loops between loops of
# different lengths, and with use_merge.
# golden: startup=empty compare=objects,context,mesh:Circle tier=T1
import bpy
bpy.ops.mesh.primitive_circle_add(vertices=8, radius=1, fill_type='NOTHING')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.primitive_circle_add(vertices=6, radius=1, fill_type='NOTHING', location=(0, 0, 1))
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bridge_edge_loops()
bpy.data.meshes['Circle'].total_face_sel
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.primitive_circle_add(vertices=8, radius=1, fill_type='NOTHING', location=(3, 0, 0))
bpy.ops.mesh.primitive_circle_add(vertices=8, radius=1.2, fill_type='NOTHING', location=(3, 0, 0.5))
bpy.ops.mesh.bridge_edge_loops(use_merge=True, merge_factor=0.5)
bpy.data.meshes['Circle'].total_face_sel
