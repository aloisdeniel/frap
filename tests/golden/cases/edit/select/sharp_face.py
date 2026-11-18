# WP 2.4: mesh.edges_select_sharp(sharpness=1.0) in face mode on an 8-sided cylinder: the cap
# rims (90 degrees) are selected and EDBM_selectmode_convert turns them into the two caps; the
# side seams (45 degrees) stay unselected.
# golden: startup=factory compare=objects,context,mesh:Cylinder tier=T1
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=8)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.edges_select_sharp(sharpness=1.0)
