# WP 2.11: a face selected in face select mode: its four edges are beveled with two
# segments; in face mode the selection is not flushed afterwards.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[3])
bpy.ops.mesh.bevel(offset=0.2, segments=2)
bpy.data.meshes['Cube'].total_face_sel
