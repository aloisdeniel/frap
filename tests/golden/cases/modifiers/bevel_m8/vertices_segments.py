# WP 8.2: BEVEL affect='VERTICES' with two, three and four segments (cube, ico
# sphere, a seven-sided cone).
# golden: startup=factory compare=evaluated:Cube,evaluated:Icosphere,evaluated:Cone tier=T1
import bpy
bpy.data.objects['Cube'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['Cube'].modifiers['Bevel'].affect = 'VERTICES'
bpy.data.objects['Cube'].modifiers['Bevel'].segments = 4
bpy.data.objects['Cube'].modifiers['Bevel'].width = 0.3
bpy.data.objects['Cube'].modifiers['Bevel'].profile = 0.8
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=1, location=(3, 0, 0))
bpy.data.objects['Icosphere'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['Icosphere'].modifiers['Bevel'].affect = 'VERTICES'
bpy.data.objects['Icosphere'].modifiers['Bevel'].segments = 3
bpy.data.objects['Icosphere'].modifiers['Bevel'].width = 0.2
bpy.ops.mesh.primitive_cone_add(vertices=7, location=(6, 0, 0))
bpy.data.objects['Cone'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['Cone'].modifiers['Bevel'].affect = 'VERTICES'
bpy.data.objects['Cone'].modifiers['Bevel'].segments = 2
bpy.data.objects['Cone'].modifiers['Bevel'].width = 0.2
