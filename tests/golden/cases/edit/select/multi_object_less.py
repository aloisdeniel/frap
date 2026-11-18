# WP 2.4: mesh.select_less shrinks the selection of every mesh in Edit Mode.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4, location=(3, 0, 0))
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 1, 2, 4, 5, 6, 8, 9, 10])
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1], object='Cube')
bpy.ops.mesh.select_less()
