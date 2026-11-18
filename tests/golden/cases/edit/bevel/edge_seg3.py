# WP 2.11: one cube edge with three segments (verified counts V14 E21 F9): a terminal
# edge at both ends (tri-fan corners), the odd middle strip straddles a UV seam.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T2 order=exact
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[0])
bpy.ops.mesh.bevel(offset=0.2, segments=3)
