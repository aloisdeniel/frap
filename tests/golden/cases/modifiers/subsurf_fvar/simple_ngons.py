# WP 8.11 (deviation D-121 closed): SUBSURF SIMPLE past level 1 with UV maps. Blender isolates
# SIMPLE to level 1, where the face point of every n-gon is extraordinary, and evaluates the UVs
# around it in face-varying Gregory end caps; frap's port of the face-varying patch builder
# gives the same f32 values. 6-sided cylinder and 5-sided cone at levels 2 and 3, a 7-gon at
# level 3 (Suzanne's triangles at SIMPLE level 2: crates/frap-subdiv tests/fvar_dumps.rs).
# golden: startup=empty compare=evaluated:Cylinder,evaluated:Cylinder.001,evaluated:Cone,evaluated:Cone.001,evaluated:Circle tier=T2 tol=1e-4
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=6)
bpy.ops.mesh.primitive_cylinder_add(vertices=6, location=(3, 0, 0))
bpy.ops.mesh.primitive_cone_add(vertices=5, location=(6, 0, 0))
bpy.ops.mesh.primitive_cone_add(vertices=5, location=(9, 0, 0))
bpy.ops.mesh.primitive_circle_add(vertices=7, fill_type='NGON', location=(12, 0, 0))
bpy.data.objects['Cylinder'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cylinder'].modifiers['Subdivision'].subdivision_type = 'SIMPLE'
bpy.data.objects['Cylinder'].modifiers['Subdivision'].levels = 2
bpy.data.objects['Cylinder.001'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cylinder.001'].modifiers['Subdivision'].subdivision_type = 'SIMPLE'
bpy.data.objects['Cylinder.001'].modifiers['Subdivision'].levels = 3
bpy.data.objects['Cone'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cone'].modifiers['Subdivision'].subdivision_type = 'SIMPLE'
bpy.data.objects['Cone'].modifiers['Subdivision'].levels = 2
bpy.data.objects['Cone.001'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cone.001'].modifiers['Subdivision'].subdivision_type = 'SIMPLE'
bpy.data.objects['Cone.001'].modifiers['Subdivision'].levels = 3
bpy.data.objects['Circle'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Circle'].modifiers['Subdivision'].subdivision_type = 'SIMPLE'
bpy.data.objects['Circle'].modifiers['Subdivision'].levels = 3
