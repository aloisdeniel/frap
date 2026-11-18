# WP 2.8: mesh.edge_face_add on a closed wire loop (circle without fill) makes one n-gon,
# oriented by recalc_face_normals (no neighbour faces), selected; then two loose vertices get
# an edge, and pressing again with the new edge selected extends nothing (CANCELLED).
# golden: startup=empty compare=objects,context,mesh:Circle tier=T2
import bpy
bpy.ops.mesh.primitive_circle_add(vertices=8, radius=1, fill_type='NOTHING')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.edge_face_add()
bpy.data.meshes['Circle'].total_face_sel
bpy.ops.mesh.primitive_circle_add(vertices=4, radius=0.3, fill_type='NOTHING', location=(3, 0, 0))
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[8, 10])
bpy.ops.mesh.edge_face_add()
bpy.ops.mesh.edge_face_add()
