# WP 2.5: mesh.extrude_region then mesh.inset on a patch of Suzanne (triangles and quads).
# golden: startup=empty compare=objects,context,mesh:Suzanne tier=T1 order=canonical
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1, 2, 3, 40, 41])
bpy.ops.mesh.extrude_repeat(steps=1, offset=(0, -0.2, 0))
bpy.ops.mesh.inset(thickness=0.02, depth=0.01)
