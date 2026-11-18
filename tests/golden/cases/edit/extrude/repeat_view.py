# WP 2.5: mesh.extrude_repeat without offset extrudes along the 3D View's direction (the
# factory Layout viewport, D-10).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1 tol=1e-4 order=canonical
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[0])
bpy.ops.mesh.extrude_repeat(steps=2)
bpy.data.meshes['Cube'].total_vert_sel
bpy.data.meshes['Cube'].total_edge_sel
bpy.data.meshes['Cube'].total_face_sel
