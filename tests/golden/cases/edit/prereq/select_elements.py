# WP 2.2: frap.select_elements by index: SET (the selection history restarts, the last index is
# the active element), ADD, REMOVE (flushed for the select mode), edges and faces in vertex
# mode, an element given twice, an empty list.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[2, 0, 5])
bpy.data.meshes['Cube'].total_vert_sel
bpy.ops.frap.select_elements(type='EDGE', indices=[3], action='ADD')
bpy.ops.frap.select_elements(type='VERT', indices=[0], action='REMOVE')
bpy.data.meshes['Cube'].total_edge_sel
bpy.ops.frap.select_elements(type='FACE', indices=[1, 4], action='ADD')
bpy.ops.frap.select_elements(type='VERT', indices=[5, 5], action='ADD')
bpy.ops.frap.select_elements(type='FACE', indices=[], action='ADD')
bpy.ops.frap.select_elements(type='EDGE', indices=[7], action='REMOVE')
