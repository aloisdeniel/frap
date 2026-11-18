# WP 2.11: one cube edge beveled with one segment (verified counts V10 E15 F7): the two old
# vertices go, the survivors keep their order, the new chamfer face is selected and the
# selection history is cleared.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[0])
bpy.ops.mesh.bevel(offset=0.2)
bpy.data.meshes['Cube'].total_vert_sel
