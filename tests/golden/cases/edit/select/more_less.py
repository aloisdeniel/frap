# WP 2.4: mesh.select_more / select_less (bmesh.ops.region_extend) on a grid in vertex mode,
# with and without use_face_step; the selection history (active vertex) survives growing and
# shrinking; a corner vertex grows and shrinks at the border.
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=6)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[14])
bpy.ops.mesh.select_more(use_face_step=False)
bpy.data.meshes['Grid'].total_vert_sel
bpy.ops.mesh.select_more()
bpy.data.meshes['Grid'].total_vert_sel
bpy.ops.mesh.select_less(use_face_step=False)
bpy.data.meshes['Grid'].total_vert_sel
bpy.ops.mesh.select_less()
bpy.data.meshes['Grid'].total_vert_sel
bpy.ops.frap.select_elements(type='VERT', indices=[0])
bpy.ops.mesh.select_more()
bpy.ops.mesh.select_more()
bpy.ops.mesh.select_less()
