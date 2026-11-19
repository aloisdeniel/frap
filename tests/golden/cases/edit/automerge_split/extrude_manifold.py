# WP 4.6: mesh.extrude_manifold (extrude_region with use_dissolve_ortho_edges, then a
# translate with use_automerge_and_split). A side face pulled out diagonally: the moved
# vertices that reach the neighbouring walls weld onto them and the walls are split.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1 tol=1e-5
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[1])
bpy.ops.mesh.extrude_manifold(TRANSFORM_OT_translate={"value": (0.3, 0.4, 0)})
bpy.data.meshes['Cube'].total_face_sel
bpy.data.meshes['Cube'].total_vert_sel
bpy.ops.object.mode_set(mode='OBJECT')
