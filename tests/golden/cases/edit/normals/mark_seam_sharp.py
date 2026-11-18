# WP 2.9: mesh.mark_seam / mark_sharp write uv_seam / sharp_edge on the selected edges;
# clearing every mark removes the attribute when Edit Mode is left ([MESH §2.5]);
# use_verts marks every edge touching a selected vertex.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 3, 5])
bpy.ops.mesh.mark_seam()
bpy.ops.mesh.mark_sharp()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.scene.cursor.location.z = bpy.data.meshes['Cube'].attributes.find('uv_seam')
bpy.data.meshes['Cube'].edges[3].use_edge_sharp
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.mark_seam(clear=True)
bpy.ops.mesh.mark_sharp(clear=True)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.scene.cursor.location.x = bpy.data.meshes['Cube'].attributes.find('uv_seam')
bpy.context.scene.cursor.location.y = bpy.data.meshes['Cube'].attributes.find('sharp_edge')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='VERT')
bpy.ops.frap.select_elements(type='VERT', indices=[7])
bpy.ops.mesh.mark_sharp(use_verts=True)
bpy.ops.mesh.mark_seam()
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.mark_seam()
bpy.ops.mesh.mark_sharp(use_verts=True)
