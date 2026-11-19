# M3+M4 stabilization round 3: SUBSURF SIMPLE level 2 on Suzanne (Gregory end caps around the
# level-1 face points of its triangles). Without UV maps (D-121); normals of the smallest faces
# amplify position rounding to ~1e-5 (D-77). DECIMATE DISSOLVE after it: D-120,
# `modifiers/bevel_decimate/dissolve_after_subsurf.py`.
# golden: startup=empty compare=evaluated:Suzanne tier=T2 tol=1e-4
import bpy
bpy.ops.mesh.primitive_monkey_add(calc_uvs=False)
bpy.data.objects['Suzanne'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Suzanne'].modifiers['Subdivision'].subdivision_type = 'SIMPLE'
bpy.data.objects['Suzanne'].modifiers['Subdivision'].levels = 2
