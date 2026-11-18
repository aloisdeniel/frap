# M4 W0 (M2 leftover, D-79): with Auto Merge on, a 0.0003 bevel of one cube edge is welded back
# right after the bevel (`EDBM_automerge_connected`), index for index as Blender.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.context.scene.tool_settings.use_mesh_automerge = True
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[0])
bpy.ops.mesh.bevel(offset=0.0003)
bpy.data.meshes['Cube'].total_vert_sel
bpy.ops.object.mode_set(mode='OBJECT')
