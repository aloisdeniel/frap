# WP 2.5: a modelling step: inset the top face, then extrude it as a region and inset again
# individually.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1 order=canonical
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[0])
bpy.ops.mesh.inset(thickness=0.25)
bpy.ops.mesh.extrude_repeat(steps=1, offset=(0, 0, 0.5))
bpy.ops.mesh.inset(thickness=0.1, use_individual=True)
bpy.data.meshes['Cube'].total_vert_sel
bpy.data.meshes['Cube'].total_edge_sel
bpy.data.meshes['Cube'].total_face_sel
