# WP 2.13 workflow (table): the top of a table slab picked by its normal, switched to edge
# select mode (its four edges stay selected) and bevelled with three segments; then the
# bottom face is inset and pushed in a little as a recess.
# golden: startup=empty compare=objects,context,mesh:Cube tier=T2
import bpy
bpy.ops.mesh.primitive_cube_add(size=2, location=(0, 0, 0.75), scale=(1, 0.6, 0.04))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_by_normal(direction=(0, 0, 1))
bpy.ops.mesh.select_mode(type='EDGE')
bpy.data.meshes['Cube'].total_edge_sel
bpy.ops.mesh.bevel(offset=0.03, segments=3)
bpy.data.meshes['Cube'].total_face_sel
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_by_normal(direction=(0, 0, -1), threshold=0.1)
bpy.ops.mesh.inset(thickness=0.1, depth=-0.01)
