# WP 2.4: mesh.faces_select_linked_flat at the default 1 degree from one triangle of a
# triangle-fan cap grows over the coplanar fan only. With nothing selected it does nothing (that line checks the outcome only).
# golden: startup=factory compare=objects,context,mesh:Cylinder tier=T1
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=8, end_fill_type='TRIFAN')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.faces_select_linked_flat()
bpy.ops.frap.select_elements(type='FACE', indices=[10])
bpy.ops.mesh.faces_select_linked_flat()
