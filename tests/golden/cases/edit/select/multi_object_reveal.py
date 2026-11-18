# WP 2.4: mesh.reveal(select=False) unhides without selecting on every mesh in Edit Mode: after
# selecting everything visible, inverting the selection leaves exactly the revealed elements.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4, location=(3, 0, 0))
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[6])
bpy.ops.frap.select_elements(type='VERT', indices=[5], object='Cube')
bpy.ops.mesh.hide()
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.reveal(select=False)
bpy.ops.mesh.select_all(action='INVERT')
