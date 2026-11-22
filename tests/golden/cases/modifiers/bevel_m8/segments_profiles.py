# WP 8.2: BEVEL with segments index-exact (T1): cube corners with three segments
# and a profile of 0.7, four segments at 0.15, a hexagonal cylinder with five segments at 0.9 and
# an ico sphere (valence-5 corners) with three segments.
# golden: startup=factory compare=evaluated:Cube,evaluated:Cube.001,evaluated:Cylinder,evaluated:Icosphere tier=T1
import bpy
bpy.data.objects['Cube'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['Cube'].modifiers['Bevel'].segments = 3
bpy.data.objects['Cube'].modifiers['Bevel'].profile = 0.7
bpy.data.objects['Cube'].modifiers['Bevel'].width = 0.2
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0))
bpy.data.objects['Cube.001'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['Cube.001'].modifiers['Bevel'].segments = 4
bpy.data.objects['Cube.001'].modifiers['Bevel'].profile = 0.15
bpy.data.objects['Cube.001'].modifiers['Bevel'].width = 0.25
bpy.ops.mesh.primitive_cylinder_add(vertices=6, location=(6, 0, 0))
bpy.data.objects['Cylinder'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['Cylinder'].modifiers['Bevel'].segments = 5
bpy.data.objects['Cylinder'].modifiers['Bevel'].profile = 0.9
bpy.data.objects['Cylinder'].modifiers['Bevel'].width = 0.15
bpy.data.objects['Cylinder'].modifiers['Bevel'].limit_method = 'NONE'
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=1, location=(9, 0, 0))
bpy.data.objects['Icosphere'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['Icosphere'].modifiers['Bevel'].segments = 3
bpy.data.objects['Icosphere'].modifiers['Bevel'].width = 0.1
bpy.data.objects['Icosphere'].modifiers['Bevel'].limit_method = 'NONE'
