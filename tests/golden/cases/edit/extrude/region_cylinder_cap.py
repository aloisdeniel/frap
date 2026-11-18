# WP 2.5: mesh.extrude_region of a cylinder's n-gon cap in face mode, moved with
# extrude_repeat afterwards.
# golden: startup=empty compare=objects,context,mesh:Cylinder tier=T1 order=canonical
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=6)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[6])
bpy.ops.mesh.extrude_region()
bpy.ops.mesh.extrude_repeat(steps=1, offset=(0, 0, 0.5))
