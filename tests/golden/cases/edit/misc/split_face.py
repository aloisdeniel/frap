# WP 2.10: mesh.split on the top face of the cube: the face is copied with four new vertices
# and the original is deleted (FACES context), so the copy takes face index 5; only the copy
# stays selected and the selection history is cleared. Then a face plus a loose selected
# vertex are split off together (the vertex is copied too); with nothing selected the
# operator does nothing.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[5])
bpy.ops.mesh.split()
bpy.data.meshes['Cube'].total_vert_sel
bpy.ops.mesh.select_mode(type='VERT')
bpy.ops.frap.select_elements(type='FACE', indices=[0])
bpy.ops.frap.select_elements(type='VERT', indices=[6], action='ADD')
bpy.ops.mesh.split()
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.split()
