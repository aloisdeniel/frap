# WP 8.2: BEVEL harden_normals with four segments on a smooth hexagonal cylinder and
# on a smooth cube (vertices, three segments).
# golden: startup=factory compare=evaluated:Cube,evaluated:Cylinder tier=T1
import bpy
bpy.ops.object.shade_smooth()
bpy.data.objects['Cube'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['Cube'].modifiers['Bevel'].affect = 'VERTICES'
bpy.data.objects['Cube'].modifiers['Bevel'].width = 0.3
bpy.data.objects['Cube'].modifiers['Bevel'].segments = 3
bpy.data.objects['Cube'].modifiers['Bevel'].harden_normals = True
bpy.ops.mesh.primitive_cylinder_add(vertices=6, location=(3, 0, 0))
bpy.ops.object.shade_smooth()
bpy.data.objects['Cylinder'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['Cylinder'].modifiers['Bevel'].width = 0.15
bpy.data.objects['Cylinder'].modifiers['Bevel'].segments = 4
bpy.data.objects['Cylinder'].modifiers['Bevel'].harden_normals = True
bpy.data.objects['Cylinder'].modifiers['Bevel'].limit_method = 'NONE'
