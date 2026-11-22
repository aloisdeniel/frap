# M3+M4 stabilization round 3: SUBSURF subdivision_type SIMPLE past level 1 with the limit
# surface. Blender isolates SIMPLE to level 1, where the face point of every non-quad is an
# extraordinary vertex, and evaluates the faces around it as Gregory end caps (frap: through the
# opensubdiv-rs patch table, decision log M34S3-1). Cone, 6-sided cylinder and 8x4 UV sphere at
# levels 2 and 3. Without UV maps (UVs in the caps: modifiers/subsurf_fvar).
# golden: startup=empty compare=evaluated:Cone,evaluated:Cone.001,evaluated:Cylinder,evaluated:Cylinder.001,evaluated:Sphere,evaluated:Sphere.001 tier=T2
import bpy
bpy.ops.mesh.primitive_cone_add(calc_uvs=False)
bpy.ops.mesh.primitive_cone_add(vertices=7, location=(3, 0, 0), calc_uvs=False)
bpy.ops.mesh.primitive_cylinder_add(vertices=6, location=(6, 0, 0), calc_uvs=False)
bpy.ops.mesh.primitive_cylinder_add(vertices=6, location=(9, 0, 0), calc_uvs=False)
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=4, location=(12, 0, 0), calc_uvs=False)
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=4, location=(15, 0, 0), calc_uvs=False)
bpy.data.objects['Cone'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cone'].modifiers['Subdivision'].subdivision_type = 'SIMPLE'
bpy.data.objects['Cone'].modifiers['Subdivision'].levels = 2
bpy.data.objects['Cone.001'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cone.001'].modifiers['Subdivision'].subdivision_type = 'SIMPLE'
bpy.data.objects['Cone.001'].modifiers['Subdivision'].levels = 3
bpy.data.objects['Cylinder'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cylinder'].modifiers['Subdivision'].subdivision_type = 'SIMPLE'
bpy.data.objects['Cylinder'].modifiers['Subdivision'].levels = 2
bpy.data.objects['Cylinder.001'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cylinder.001'].modifiers['Subdivision'].subdivision_type = 'SIMPLE'
bpy.data.objects['Cylinder.001'].modifiers['Subdivision'].levels = 3
bpy.data.objects['Sphere'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Sphere'].modifiers['Subdivision'].subdivision_type = 'SIMPLE'
bpy.data.objects['Sphere'].modifiers['Subdivision'].levels = 2
bpy.data.objects['Sphere.001'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Sphere.001'].modifiers['Subdivision'].subdivision_type = 'SIMPLE'
bpy.data.objects['Sphere.001'].modifiers['Subdivision'].levels = 3
