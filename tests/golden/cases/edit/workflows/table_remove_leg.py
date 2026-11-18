# WP 2.13 workflow (table, selection recipes): a table built from five cubes in one mesh; one
# leg is picked by a single vertex, grown to its whole island with mesh.select_linked and
# deleted; then the top is picked by a vertex, grown by one ring with mesh.select_more,
# shrunk back with mesh.select_less, and the island selection inverted.
# golden: startup=empty compare=objects,context,mesh:Table tier=T1
import bpy
bpy.ops.mesh.primitive_cube_add(size=2, location=(0, 0, 0.75), scale=(1, 0.6, 0.04))
bpy.context.object.name = 'Table'
bpy.context.object.data.name = 'Table'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.primitive_cube_add(size=0.1, location=(0.9, 0.5, 0.36), scale=(1, 1, 7.2))
bpy.ops.mesh.primitive_cube_add(size=0.1, location=(-0.9, 0.5, 0.36), scale=(1, 1, 7.2))
bpy.ops.mesh.primitive_cube_add(size=0.1, location=(-0.9, -0.5, 0.36), scale=(1, 1, 7.2))
bpy.ops.mesh.primitive_cube_add(size=0.1, location=(0.9, -0.5, 0.36), scale=(1, 1, 7.2))
bpy.ops.frap.select_elements(type='VERT', indices=[17])
bpy.ops.mesh.select_linked()
bpy.data.meshes['Table'].total_vert_sel
bpy.ops.mesh.delete(type='VERT')
bpy.ops.frap.select_elements(type='VERT', indices=[0])
bpy.ops.mesh.select_more()
bpy.data.meshes['Table'].total_vert_sel
bpy.ops.mesh.select_less()
bpy.data.meshes['Table'].total_vert_sel
bpy.ops.mesh.select_linked()
bpy.ops.mesh.select_all(action='INVERT')
bpy.data.meshes['Table'].total_vert_sel
