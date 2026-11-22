# WP 2.11: vertex bevel of some Suzanne vertices with two segments.
# golden: startup=factory compare=mesh:Suzanne tier=T1
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 5, 17, 46, 100, 250])
bpy.ops.mesh.bevel(offset=0.02, segments=2, affect='VERTICES')
