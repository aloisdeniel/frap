# WP 3.8: uv.pin pins the selected UVs (.pn.UVMap); pinned UVs are constraints of uv.unwrap
# (CONFORMAL) and their island is not packed; invert toggles, clear removes the pins.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T2 tol=1e-4
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 2, 3, 5, 6, 9, 11])
bpy.ops.mesh.mark_seam()
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_mode(type='VERT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 1])
bpy.ops.uv.pin()
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.unwrap(method='CONFORMAL')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.scene.cursor.location.x = bpy.data.meshes['Cube'].attributes['.pn.UVMap'].data[0].value
bpy.context.scene.cursor.location.y = bpy.data.meshes['Cube'].attributes['.pn.UVMap'].data[1].value
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 2])
bpy.ops.uv.pin(invert=True)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.scene.cursor.location.z = bpy.data.meshes['Cube'].attributes['.pn.UVMap'].data[0].value
bpy.data.objects['Cube'].location.x = bpy.data.meshes['Cube'].attributes['.pn.UVMap'].data[4].value
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.pin(clear=True)
bpy.ops.uv.unwrap(method='CONFORMAL')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Cube'].location.y = bpy.data.meshes['Cube'].attributes.find('.pn.UVMap')
