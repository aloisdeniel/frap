# WP 2.4: mesh.select_more grows the selection of every mesh in Edit Mode.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4, location=(3, 0, 0))
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[5])
bpy.ops.frap.select_elements(type='VERT', indices=[0], object='Cube')
bpy.ops.mesh.select_more()
