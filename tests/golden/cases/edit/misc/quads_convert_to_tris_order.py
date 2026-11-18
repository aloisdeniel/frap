# WP 2.10: mesh.quads_convert_to_tris(BEAUTY) on the whole cube compared index by index
# (T1): on the cube's square faces both diagonals tie and the order of the slots (each
# face's last triangle in its own slot, the first appended) is Blender's ([MESH §4.13]).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.quads_convert_to_tris()
bpy.data.meshes['Cube'].total_face_sel
