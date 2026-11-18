# WP 2.10: mesh.duplicate copies the selected face, its edges and vertices (appended), selects
# only the copies and moves the selection history to them; the copy is not moved. With
# nothing selected the operator is cancelled.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[5, 2])
bpy.ops.mesh.duplicate()
bpy.data.meshes['Cube'].total_vert_sel
bpy.ops.frap.select_elements(type='VERT', indices=[1])
bpy.ops.frap.select_elements(type='EDGE', indices=[3], action='ADD')
bpy.ops.mesh.duplicate(mode=1)
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.duplicate()
