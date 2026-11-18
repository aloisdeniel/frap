# WP 2.13 workflow (cup): two circles, the second added in Edit Mode higher and wider, both
# selected and joined into a tapered wall with mesh.bridge_edge_loops; the bottom circle is
# then picked by index and filled with mesh.edge_face_add, and the normals made consistent.
# golden: startup=empty compare=objects,context,mesh:Circle tier=T1
import bpy
bpy.ops.mesh.primitive_circle_add(vertices=16, radius=0.3)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.primitive_circle_add(vertices=16, radius=0.45, location=(0, 0, 0.9))
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bridge_edge_loops()
bpy.data.meshes['Circle'].total_face_sel
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15])
bpy.ops.mesh.edge_face_add()
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.normals_make_consistent(inside=False)
