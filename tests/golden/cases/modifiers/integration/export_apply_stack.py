# WP 4.7: export_apply=True writes every object's viewport-evaluated mesh: a modifier stack
# per object (Bevel then Subdivision with UV smoothing, Array then Mirror, Solidify,
# Triangulate, Wireframe), a modifier off in the viewport but on in renders (skipped), and an
# object without modifiers (its data). Positions, normals and UVs follow Blender's.
# golden: startup=factory tier=T2 compare=glb:out/stack.glb tol=1e-4 glb_structure
import bpy
bpy.data.objects['Cube'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['Cube'].modifiers['Bevel'].width = 0.2
bpy.data.objects['Cube'].modifiers['Bevel'].segments = 2
bpy.data.objects['Cube'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cube'].modifiers['Subdivision'].levels = 1
bpy.ops.mesh.primitive_plane_add(size=1, location=(3, 0, 0))
bpy.data.objects['Plane'].modifiers.new(name='Array', type='ARRAY')
bpy.data.objects['Plane'].modifiers['Array'].count = 3
bpy.data.objects['Plane'].modifiers.new(name='Mirror', type='MIRROR')
bpy.data.objects['Plane'].modifiers['Mirror'].use_axis = (False, True, False)
bpy.ops.mesh.primitive_plane_add(size=1, location=(0, 3, 0))
bpy.data.objects['Plane.001'].modifiers.new(name='Solidify', type='SOLIDIFY')
bpy.data.objects['Plane.001'].modifiers['Solidify'].thickness = 0.3
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=5, location=(-3, 0, 0))
bpy.data.objects['Sphere'].modifiers.new(name='Triangulate', type='TRIANGULATE')
bpy.data.objects['Sphere'].modifiers.new(name='Off', type='SUBSURF')
bpy.data.objects['Sphere'].modifiers['Off'].show_viewport = False
bpy.ops.mesh.primitive_cylinder_add(vertices=6, location=(0, -3, 0))
bpy.data.objects['Cylinder'].modifiers.new(name='Wireframe', type='WIREFRAME')
bpy.ops.mesh.primitive_cone_add(vertices=5, location=(-3, -3, 0))
bpy.ops.export_scene.gltf(filepath='out/stack.glb', export_apply=True)
