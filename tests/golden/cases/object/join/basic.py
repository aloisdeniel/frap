# WP 1.20 (T0), object.join: a grid and a smooth-shaded plane are joined into the moved and
# scaled Cube (vertices mapped into the Cube's space, Cube first, then base order); the selected
# camera is ignored and stays selected; the grid and plane objects are deleted and their meshes
# kept as orphans; the joined faces without material get one empty slot. (No UV sphere: its
# edge order is not stable in Blender, D-19.)
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Grid tier=T0
import bpy
bpy.ops.object.select_all(action='DESELECT')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=3, y_subdivisions=2, size=1, location=(3, 0, 0))
bpy.ops.mesh.primitive_plane_add(location=(0, 3, 0), rotation=(0.5, 0, 0))
bpy.data.meshes['Plane'].shade_smooth()
bpy.data.objects['Grid'].select_set(True)
bpy.data.objects['Cube'].select_set(True)
bpy.data.objects['Camera'].select_set(True)
bpy.data.objects['Cube'].location = (0, 0, 1)
bpy.data.objects['Cube'].scale = (2, 1, 1)
bpy.data.objects['Cube'].rotation_euler = (0, 0, 0.3)
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.join()
bpy.data.objects['Grid']
bpy.data.meshes['Plane']
bpy.data.meshes['Cube'].materials[1]
