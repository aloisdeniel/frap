# WP 2.5: mesh.extrude_repeat(steps=3, offset=(0, 0, 0.5)) on the top face: three stacked region
# extrusions moved up by 0.5 each (V20 E36 F18 on the cube); the last cap is selected.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1 order=canonical
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[0])
bpy.ops.mesh.extrude_repeat(steps=3, offset=(0, 0, 0.5))
bpy.data.meshes['Cube'].total_vert_sel
bpy.data.meshes['Cube'].total_edge_sel
bpy.data.meshes['Cube'].total_face_sel
