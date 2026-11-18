# WP 4.3: SIMPLE_DEFORM TWIST in the space of a rotated, moved empty (`origin`).
# golden: startup=empty compare=evaluated:Cylinder tier=T1
import bpy
bpy.ops.object.empty_add(location=(0.4, 0.1, 0.5), rotation=(0.5, 0.2, 0))
bpy.ops.mesh.primitive_cylinder_add(vertices=12, depth=3)
bpy.data.objects['Cylinder'].modifiers.new(name='M', type='SIMPLE_DEFORM')
bpy.data.objects['Cylinder'].modifiers['M'].deform_axis = 'Z'
bpy.data.objects['Cylinder'].modifiers['M'].angle = 0.9
bpy.data.objects['Cylinder'].modifiers['M'].origin = bpy.data.objects['Empty']
