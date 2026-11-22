# WP 8.6: mesh.intersect(solver='FLOAT', separate_mode='NONE', threshold=0.01): at the RNA
# maximum merge distance the cut of a rotated cube through the default cube snaps onto corners,
# so the island connection takes the partial-connect split and splice
# (bm_face_split_edgenet_partial_connect); this input used to loop forever.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T2 tol=1e-4
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.primitive_cube_add(size=1.2, location=(0.7, 0.4, 0.3), rotation=(0.3, 0.2, 0.1))
bpy.ops.mesh.intersect(solver='FLOAT', separate_mode='NONE', threshold=0.01)
