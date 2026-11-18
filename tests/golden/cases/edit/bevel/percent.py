# WP 2.11: offset_type PERCENT reads offset_pct (verified counts V10 E15 F7).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[0])
bpy.ops.mesh.bevel(offset_type='PERCENT', offset_pct=25, offset=0.9)
