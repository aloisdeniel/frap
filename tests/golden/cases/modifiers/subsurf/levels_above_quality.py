# M3+M4 stabilization round 3: SUBSURF levels above quality. Blender isolates to `quality` and
# evaluates the faces around extraordinary vertices as Gregory end caps between the isolation
# level's vertices (frap: through the opensubdiv-rs patch table, decision log M34S3-1): level 4
# at the default quality 3, level 3 at quality 1; a 7-gon and a grid add smooth boundary corners
# (Blender's legacy sharp-corner patches at the isolation level). Without UV maps (UVs:
# modifiers/subsurf_fvar); small level-4 face normals amplify position rounding to ~1e-5 (D-77).
# golden: startup=empty compare=evaluated:Cylinder,evaluated:Sphere,evaluated:Cube,evaluated:Circle,evaluated:Grid tier=T2 tol=1e-4
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=6, calc_uvs=False)
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=4, location=(3, 0, 0), calc_uvs=False)
bpy.ops.mesh.primitive_cube_add(location=(6, 0, 0), calc_uvs=False)
bpy.ops.mesh.primitive_circle_add(vertices=7, fill_type='NGON', location=(9, 0, 0), calc_uvs=False)
bpy.ops.mesh.primitive_grid_add(x_subdivisions=3, y_subdivisions=2, location=(12, 0, 0), calc_uvs=False)
bpy.data.objects['Cylinder'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cylinder'].modifiers['Subdivision'].levels = 4
bpy.data.objects['Sphere'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Sphere'].modifiers['Subdivision'].levels = 4
bpy.data.objects['Cube'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cube'].modifiers['Subdivision'].levels = 3
bpy.data.objects['Cube'].modifiers['Subdivision'].quality = 1
bpy.data.objects['Circle'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Circle'].modifiers['Subdivision'].levels = 4
bpy.data.objects['Grid'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Grid'].modifiers['Subdivision'].levels = 4
