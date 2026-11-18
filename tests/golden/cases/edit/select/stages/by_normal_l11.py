# WP 2.4: edit/select/by_normal.py cut after its line 11,
#   bpy.ops.frap.select_by_normal(direction=(0, 0, 1), orientation='LOCAL')
# so that call's effect reaches the dump (later lines of the full case replace it).
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Grid tier=T1
import bpy
bpy.data.objects['Cube'].rotation_euler = (3.14159265, 0, 0.3)
bpy.ops.mesh.primitive_grid_add(x_subdivisions=3, y_subdivisions=3, location=(3, 0, 0))
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_by_normal()
bpy.ops.frap.select_by_normal(direction=(0, 0, 1), orientation='LOCAL')
