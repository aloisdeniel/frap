# WP 8.2: affect='VERTICES' with four segments on some Suzanne vertices.
# golden: startup=factory compare=objects,context,mesh:Suzanne tier=T1
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 1, 10, 20, 46, 100, 200, 300, 400])
bpy.ops.mesh.bevel(offset=0.02, segments=4, affect='VERTICES')
