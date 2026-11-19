# WP 3.8: uv.mark_seam marks the UV-selected edges (sync selection: edge mode marks the
# selected edges, vertex mode the edges with both vertices selected, face mode the edges of
# selected faces and of faces sharing their UVs); clear=True unmarks.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 4])
bpy.ops.uv.mark_seam()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.scene.cursor.location.x = bpy.data.meshes['Cube'].edges[4].use_seam
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_mode(type='VERT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 1, 3])
bpy.ops.uv.mark_seam()
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[5])
bpy.ops.uv.mark_seam()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.scene.cursor.location.y = bpy.data.meshes['Cube'].edges[7].use_seam
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[1])
bpy.ops.uv.mark_seam(clear=True)
bpy.ops.object.mode_set(mode='OBJECT')
