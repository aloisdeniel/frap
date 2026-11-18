# WP 2.4: mesh.region_to_loop (the boundary edges of the selected faces; face-only select
# mode switches to edge mode and tool_settings.mesh_select_mode follows) and
# mesh.loop_to_region (the smaller, or with select_bigger the bigger, region inside a loop).
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=6)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[6, 7, 8, 11, 12, 13])
bpy.ops.mesh.region_to_loop()
bpy.data.meshes['Grid'].total_edge_sel
bpy.context.scene.tool_settings.mesh_select_mode[:]
bpy.ops.mesh.loop_to_region()
bpy.data.meshes['Grid'].total_face_sel
bpy.ops.mesh.region_to_loop()
bpy.ops.mesh.loop_to_region(select_bigger=True)
bpy.data.meshes['Grid'].total_face_sel
bpy.ops.mesh.select_mode(type='VERT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 12])
bpy.ops.mesh.region_to_loop()
bpy.ops.frap.select_elements(type='FACE', indices=[0])
bpy.ops.mesh.region_to_loop()
bpy.data.meshes['Grid'].total_edge_sel
