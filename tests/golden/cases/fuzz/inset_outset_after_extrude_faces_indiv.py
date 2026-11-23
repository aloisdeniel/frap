# M8 stabilization round 3 (fuzz seed 0xb85755713edef4b, run 0x8e417e57 trial 7628): an outset
# with depth over the zero-area walls of an in-place `extrude_faces_indiv`, after a mode toggle.
# Entering Edit Mode recomputes the BMesh normals (`BKE_editmesh_looptris_and_normals_calc`):
# the walls' normals are zero there, so the caps' vertex normals are the caps' own, not the
# Mesh normals (the Mesh gives a zero-area face (0, 0, 1)) that frap used to copy.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T2 tol=0.0001
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.extrude_faces_indiv()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.inset(thickness=0.33, depth=-0.15, use_even_offset=False, use_edge_rail=False, use_outset=True, use_select_inset=False)
