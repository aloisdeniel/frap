# WP 8.11 (deviation D-121 closed): SUBSURF levels above quality with UV maps. Blender
# isolates to `quality` and evaluates the points between the isolation level's values in
# face-varying Gregory end caps (local points shared between adjacent caps): a 7-gon at level 4,
# the 6-sided cylinder at level 3 with quality 2, an 8 x 4 UV sphere at level 3 with quality 2,
# the cube at level 3 with quality 1, a grid with smooth boundary corners at level 3 with
# quality 1. Normals of the small faces amplify position rounding (D-77).
# golden: startup=empty compare=evaluated:Circle,evaluated:Cylinder,evaluated:Sphere,evaluated:Cube,evaluated:Grid tier=T2 tol=1e-4
import bpy
bpy.ops.mesh.primitive_circle_add(vertices=7, fill_type='NGON')
bpy.ops.mesh.primitive_cylinder_add(vertices=6, location=(3, 0, 0))
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=4, location=(6, 0, 0))
bpy.ops.mesh.primitive_cube_add(location=(9, 0, 0))
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=3, location=(12, 0, 0))
bpy.data.objects['Circle'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Circle'].modifiers['Subdivision'].levels = 4
bpy.data.objects['Cylinder'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cylinder'].modifiers['Subdivision'].levels = 3
bpy.data.objects['Cylinder'].modifiers['Subdivision'].quality = 2
bpy.data.objects['Sphere'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Sphere'].modifiers['Subdivision'].levels = 3
bpy.data.objects['Sphere'].modifiers['Subdivision'].quality = 2
bpy.data.objects['Cube'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cube'].modifiers['Subdivision'].levels = 3
bpy.data.objects['Cube'].modifiers['Subdivision'].quality = 1
bpy.data.objects['Grid'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Grid'].modifiers['Subdivision'].levels = 3
bpy.data.objects['Grid'].modifiers['Subdivision'].quality = 1
