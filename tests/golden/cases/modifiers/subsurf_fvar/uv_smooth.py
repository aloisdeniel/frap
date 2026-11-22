# WP 8.11: every SUBSURF uv_smooth (linear, corners, junctions, concave corners, boundaries,
# smooth everywhere) on the 6-sided cylinder at level 2, whose UV map has a seam, boundary
# islands and n-gon caps: the channel's linear-interpolation option drives OpenSubdiv's value
# tags, the adaptive isolation of UV seams and the face-varying patch types, all ported bit for
# bit (Suzanne in every mode: crates/frap-subdiv tests/fvar_dumps.rs).
# golden: startup=empty compare=evaluated:Cylinder,evaluated:Cylinder.001,evaluated:Cylinder.002,evaluated:Cylinder.003,evaluated:Cylinder.004,evaluated:Cylinder.005 tier=T1 tol=1e-4
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=6)
bpy.ops.mesh.primitive_cylinder_add(vertices=6, location=(3, 0, 0))
bpy.ops.mesh.primitive_cylinder_add(vertices=6, location=(6, 0, 0))
bpy.ops.mesh.primitive_cylinder_add(vertices=6, location=(9, 0, 0))
bpy.ops.mesh.primitive_cylinder_add(vertices=6, location=(12, 0, 0))
bpy.ops.mesh.primitive_cylinder_add(vertices=6, location=(15, 0, 0))
bpy.data.objects['Cylinder'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cylinder'].modifiers['Subdivision'].levels = 2
bpy.data.objects['Cylinder'].modifiers['Subdivision'].uv_smooth = 'NONE'
bpy.data.objects['Cylinder.001'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cylinder.001'].modifiers['Subdivision'].levels = 2
bpy.data.objects['Cylinder.001'].modifiers['Subdivision'].uv_smooth = 'PRESERVE_CORNERS'
bpy.data.objects['Cylinder.002'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cylinder.002'].modifiers['Subdivision'].levels = 2
bpy.data.objects['Cylinder.002'].modifiers['Subdivision'].uv_smooth = 'PRESERVE_CORNERS_AND_JUNCTIONS'
bpy.data.objects['Cylinder.003'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cylinder.003'].modifiers['Subdivision'].levels = 2
bpy.data.objects['Cylinder.003'].modifiers['Subdivision'].uv_smooth = 'PRESERVE_CORNERS_JUNCTIONS_AND_CONCAVE'
bpy.data.objects['Cylinder.004'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cylinder.004'].modifiers['Subdivision'].levels = 2
bpy.data.objects['Cylinder.004'].modifiers['Subdivision'].uv_smooth = 'PRESERVE_BOUNDARIES'
bpy.data.objects['Cylinder.005'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cylinder.005'].modifiers['Subdivision'].levels = 2
bpy.data.objects['Cylinder.005'].modifiers['Subdivision'].uv_smooth = 'SMOOTH_ALL'
