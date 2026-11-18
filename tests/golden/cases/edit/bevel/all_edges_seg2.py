# WP 2.11: every cube edge beveled with two segments (verified counts V56 E108 F54): the
# corners use the cube-corner (tri-corner) vertex mesh.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T2
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bevel(offset=0.2, segments=2)
bpy.data.meshes['Cube'].total_face_sel
