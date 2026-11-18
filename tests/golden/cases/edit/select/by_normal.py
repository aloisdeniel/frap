# WP 2.4: frap.select_by_normal (Blender side: tools/blender/emulations.py) on a rotated cube
# and a grid in multi-object Edit Mode: GLOBAL compares world-space normals, LOCAL each
# object's local ones; extend keeps the selection; a zero direction raises ValueError.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Grid tier=T1
import bpy
bpy.data.objects['Cube'].rotation_euler = (3.14159265, 0, 0.3)
bpy.ops.mesh.primitive_grid_add(x_subdivisions=3, y_subdivisions=3, location=(3, 0, 0))
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_by_normal()
bpy.ops.frap.select_by_normal(direction=(0, 0, 1), orientation='LOCAL')
bpy.ops.frap.select_by_normal(direction=(1, 0, 0), threshold=1.2, extend=True)
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_by_normal(direction=(0, 0, -2), threshold=0.1)
bpy.ops.frap.select_by_normal(direction=(0, 0, 0))
