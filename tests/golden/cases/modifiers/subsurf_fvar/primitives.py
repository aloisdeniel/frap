# WP 8.11: SUBSURF UV maps of the cube (level 3), a 16 x 8 UV sphere (level 2) and a 12-sided
# cylinder with its n-gon caps (level 2), evaluated in OpenSubdiv's face-varying patches as
# Blender does (seams, the poles' fans, the caps' irregular face points). T2: the UV sphere's
# edge and face order vary between Blender runs (D-19).
# golden: startup=empty compare=evaluated:Cube,evaluated:Sphere,evaluated:Cylinder tier=T2 tol=1e-4
import bpy
bpy.ops.mesh.primitive_cube_add()
bpy.ops.mesh.primitive_uv_sphere_add(segments=16, ring_count=8, location=(3, 0, 0))
bpy.ops.mesh.primitive_cylinder_add(vertices=12, location=(6, 0, 0))
bpy.data.objects['Cube'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cube'].modifiers['Subdivision'].levels = 3
bpy.data.objects['Sphere'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Sphere'].modifiers['Subdivision'].levels = 2
bpy.data.objects['Cylinder'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cylinder'].modifiers['Subdivision'].levels = 2
