# WP 2.4: mesh.hide and mesh.select_linked act on every mesh in Edit Mode; hidden elements
# stop select_linked.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4, location=(3, 0, 0))
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[6, 7, 8])
bpy.ops.frap.select_elements(type='VERT', indices=[5], object='Cube')
bpy.ops.mesh.hide()
bpy.ops.frap.select_elements(type='VERT', indices=[1])
bpy.ops.frap.select_elements(type='VERT', indices=[2], object='Cube')
bpy.ops.mesh.select_linked()
