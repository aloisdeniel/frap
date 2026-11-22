# WP 8.11: SUBSURF UV maps through OpenSubdiv's face-varying patches, bit for bit: Suzanne at
# level 1 (the default PRESERVE_BOUNDARIES, seams through the eye holes and the nose's
# valence-2 vertex). frap's UVs are Blender's f32 values exactly (crates/frap-subdiv
# tests/fvar_dumps.rs compares the bits, also at level 2); positions keep T2 accuracy and the
# normals of the smallest faces amplify it (D-77), hence the tolerance.
# golden: startup=empty compare=evaluated:Suzanne tier=T1 tol=1e-4
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.data.objects['Suzanne'].modifiers.new(name='Subdivision', type='SUBSURF')
