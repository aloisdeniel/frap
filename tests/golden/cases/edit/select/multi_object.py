# WP 2.4: selection operators in multi-object Edit Mode: loop_select on the second object
# (object_index=1) clears the selection of the others; select_linked, select_more and hide /
# reveal act on every mesh in Edit Mode; hidden elements stop select_linked.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4, location=(3, 0, 0))
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.loop_select(object_index=1, edge_index=4)
bpy.data.meshes['Cube'].total_vert_sel
bpy.data.meshes['Grid'].total_vert_sel
bpy.ops.mesh.loop_select(object_index=0, edge_index=5, extend=True)
bpy.data.meshes['Cube'].total_vert_sel
bpy.ops.mesh.select_more()
bpy.ops.frap.select_elements(type='VERT', indices=[0])
bpy.ops.frap.select_elements(type='VERT', indices=[5], object='Cube')
bpy.ops.mesh.hide()
bpy.ops.frap.select_elements(type='VERT', indices=[1])
bpy.ops.frap.select_elements(type='VERT', indices=[2], object='Cube')
bpy.ops.mesh.select_linked()
bpy.data.meshes['Grid'].total_vert_sel
bpy.data.meshes['Cube'].total_vert_sel
bpy.ops.mesh.reveal(select=False)
bpy.ops.mesh.select_less()
