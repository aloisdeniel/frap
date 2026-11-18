# WP 2.4: mesh.faces_select_linked_flat(sharpness=0.9) from one side quad of an 8-sided
# cylinder grows over every side (45 degrees apart) but not onto the caps (90 degrees).
# golden: startup=factory compare=objects,context,mesh:Cylinder tier=T1
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=8)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[0])
bpy.ops.mesh.faces_select_linked_flat(sharpness=0.9)
