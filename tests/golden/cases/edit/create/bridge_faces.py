# WP 2.8: mesh.bridge_edge_loops with faces selected: two opposite faces of a cube are
# deleted (keeping their boundary) and their loops bridged into a tunnel, wound like the
# surrounding faces.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 2])
bpy.ops.mesh.bridge_edge_loops()
bpy.data.meshes['Cube'].total_face_sel
