# WP 2.5: mesh.extrude_region(use_dissolve_ortho_edges=True) on the top face: every side quad is
# perpendicular to the cap and folds into the wall below (V8 E12 F6, the extrude_manifold step).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1 order=canonical
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[0])
bpy.ops.mesh.extrude_region(use_dissolve_ortho_edges=True)
bpy.data.meshes['Cube'].total_vert_sel
bpy.data.meshes['Cube'].total_edge_sel
bpy.data.meshes['Cube'].total_face_sel
