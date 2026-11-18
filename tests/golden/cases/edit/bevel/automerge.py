# M4 W0 (M2 leftover, D-79): with Auto Merge on, mesh.bevel welds the selection into connected
# vertices within double_threshold afterwards (`EDBM_automerge_connected`): a 0.0003 bevel of
# one cube edge collapses again (its two new vertices at each end are 0.0004 apart, under the
# default 0.001), a 0.2 bevel is kept. T2: a bevel after a weld orders its new faces differently
# from Blender (the weld's freed BMesh slots, M34W0 follow-up: the same happens after
# mesh.remove_doubles); `automerge_single` pins the weld itself at T1.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T2
import bpy
bpy.context.scene.tool_settings.use_mesh_automerge = True
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[0])
bpy.ops.mesh.bevel(offset=0.0003)
bpy.data.meshes['Cube'].total_vert_sel
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[3])
bpy.ops.mesh.bevel(offset=0.2)
bpy.data.meshes['Cube'].total_vert_sel
bpy.ops.object.mode_set(mode='OBJECT')
