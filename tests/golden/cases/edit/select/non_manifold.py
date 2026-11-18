# WP 2.4: mesh.select_non_manifold on a grid (its border), with extend and the per-kind
# toggles, on a closed cube (nothing), and its poll in face-only select mode (Blender's message).
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=3)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_non_manifold()
bpy.data.meshes['Grid'].total_vert_sel
bpy.data.meshes['Grid'].total_edge_sel
bpy.ops.frap.select_elements(type='VERT', indices=[7])
bpy.ops.mesh.select_non_manifold(extend=False, use_boundary=False)
bpy.data.meshes['Grid'].total_vert_sel
bpy.ops.mesh.select_non_manifold(extend=False, use_verts=False)
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.mesh.select_non_manifold(extend=False, use_wire=False, use_multi_face=False)
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.mesh.select_non_manifold()
bpy.ops.mesh.select_mode(type='VERT')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.select_non_manifold()
