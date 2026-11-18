# WP 2.4: mesh.edges_select_sharp at the default 30 degrees in vertex mode on a 16-sided
# cylinder: the cap rims (90 degrees) are selected, the side seams (22.5 degrees) are not.
# golden: startup=factory compare=objects,context,mesh:Cylinder tier=T1
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=16)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.edges_select_sharp()
