# WP 2.5: mesh.extrude_repeat converts the world-space offset into object space (scaled and
# rotated object) and scales it by scale_offset.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1 tol=1e-5 order=canonical
import bpy
bpy.data.objects['Cube'].scale = (2, 1, 0.5)
bpy.data.objects['Cube'].rotation_euler = (0, 0, 0.5)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[4])
bpy.ops.mesh.extrude_repeat(steps=2, offset=(1, 0, 0), scale_offset=0.5)
bpy.data.meshes['Cube'].total_vert_sel
bpy.data.meshes['Cube'].total_edge_sel
bpy.data.meshes['Cube'].total_face_sel
