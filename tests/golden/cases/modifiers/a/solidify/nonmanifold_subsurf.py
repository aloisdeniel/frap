# M3+M4 stabilization round 4 (D-122): SOLIDIFY "Complex" (NON_MANIFOLD, CONSTRAINTS thickness
# mode) followed by SUBSURF. On a cube and a plane frap matches Blender to 1e-6. On Suzanne
# after a bisecting MIRROR, the positions differ by float rounding (~1e-7, accumulation order of
# the bisect and of the NON_MANIFOLD offsets) and SUBSURF's limit normals amplify it at a few
# near-degenerate corners: of about 15k corners, the 4 of one face are up to 3.1e-4 away, 8 are
# above 2e-4 and a few dozen above 2e-5 (measured on 5.2.4). Compared at tol=4e-4 (D-122).
# golden: startup=empty compare=evaluated:Suzanne,evaluated:Cube,evaluated:Plane tier=T2 tol=4e-4
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.data.objects['Suzanne'].modifiers.new(name='Mirror', type='MIRROR')
bpy.data.objects['Suzanne'].modifiers['Mirror'].use_bisect_axis[0] = True
bpy.data.objects['Suzanne'].modifiers.new(name='Solidify', type='SOLIDIFY')
bpy.data.objects['Suzanne'].modifiers['Solidify'].solidify_mode = 'NON_MANIFOLD'
bpy.data.objects['Suzanne'].modifiers['Solidify'].thickness = 0.05
bpy.data.objects['Suzanne'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.ops.mesh.primitive_cube_add(location=(4, 0, 0))
bpy.data.objects['Cube'].modifiers.new(name='Solidify', type='SOLIDIFY')
bpy.data.objects['Cube'].modifiers['Solidify'].solidify_mode = 'NON_MANIFOLD'
bpy.data.objects['Cube'].modifiers['Solidify'].thickness = 0.2
bpy.data.objects['Cube'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.ops.mesh.primitive_plane_add(location=(-4, 0, 0))
bpy.data.objects['Plane'].modifiers.new(name='Solidify', type='SOLIDIFY')
bpy.data.objects['Plane'].modifiers['Solidify'].solidify_mode = 'NON_MANIFOLD'
bpy.data.objects['Plane'].modifiers['Solidify'].thickness = 0.1
bpy.data.objects['Plane'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Plane'].modifiers['Subdivision'].levels = 2
