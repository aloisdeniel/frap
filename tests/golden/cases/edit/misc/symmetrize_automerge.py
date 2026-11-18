# M4 W0 (M2 leftover): with Auto Merge on, mesh.symmetrize welds the selected result into
# connected vertices within double_threshold afterwards (`EDBM_automerge_connected`): the tiny
# bevel made with Auto Merge off (edges 0.0004 long) is welded away by the symmetrize.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T2
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[0])
bpy.ops.mesh.bevel(offset=0.0003)
bpy.context.scene.tool_settings.use_mesh_automerge = True
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.symmetrize(direction='NEGATIVE_X')
bpy.data.meshes['Cube'].total_vert_sel
bpy.ops.object.mode_set(mode='OBJECT')
