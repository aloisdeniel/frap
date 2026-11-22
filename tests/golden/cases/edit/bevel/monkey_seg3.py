# WP 2.11: some edges of Suzanne with three segments (odd: the middle strip and the
# center polygons straddle UV islands).
# golden: startup=factory compare=mesh:Suzanne tier=T1
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 1, 2, 3, 10, 11, 12, 40, 41, 42, 43, 100, 101, 102, 200, 201, 300])
bpy.ops.mesh.bevel(offset=0.03, segments=3)
