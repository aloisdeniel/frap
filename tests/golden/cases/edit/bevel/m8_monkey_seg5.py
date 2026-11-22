# WP 8.2: some edges of Suzanne with five segments and a profile of 0.3 (irregular
# valence-3 and valence-4 corners, UV islands).
# golden: startup=factory compare=objects,context,mesh:Suzanne tier=T1
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 1, 2, 3, 10, 11, 12, 40, 41, 42, 43, 100, 101, 102, 200, 201, 300, 500, 501, 502, 700])
bpy.ops.mesh.bevel(offset=0.02, segments=5, profile=0.3)
