# WP 4.3: SIMPLE_DEFORM TWIST (the default method) about Z with `angle` set, on a cylinder;
# the default deform axis X on a second one.
# golden: startup=empty compare=evaluated:Cylinder,evaluated:Cylinder.001 tier=T1
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=12, depth=3)
bpy.data.objects['Cylinder'].modifiers.new(name='M', type='SIMPLE_DEFORM')
bpy.data.objects['Cylinder'].modifiers['M'].deform_axis = 'Z'
bpy.data.objects['Cylinder'].modifiers['M'].angle = 1.2
bpy.ops.mesh.primitive_cylinder_add(vertices=12, depth=3, location=(4, 0, 0))
bpy.data.objects['Cylinder.001'].modifiers.new(name='M', type='SIMPLE_DEFORM')
