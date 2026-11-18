# WP 4.5: DECIMATE UNSUBDIV (a grid twice, Suzanne once) and DISSOLVE ("Planar": Suzanne at
# 0.3 rad, Suzanne at 0.6 rad delimited by UV seams, the cylinder at the default 5°, and with
# all boundary vertices dissolved); iterations 0 (the default) passes the cube through.
# golden: startup=factory compare=evaluated:Grid,evaluated:Suzanne,evaluated:Suzanne.001,evaluated:Suzanne.002,evaluated:Cylinder,evaluated:Cylinder.001,evaluated:Cube tier=T1
import bpy
bpy.data.objects['Cube'].modifiers.new(name='Decimate', type='DECIMATE')
bpy.data.objects['Cube'].modifiers['Decimate'].decimate_type = 'UNSUBDIV'
bpy.ops.mesh.primitive_grid_add(location=(0, -4, 0))
bpy.data.objects['Grid'].modifiers.new(name='Decimate', type='DECIMATE')
bpy.data.objects['Grid'].modifiers['Decimate'].decimate_type = 'UNSUBDIV'
bpy.data.objects['Grid'].modifiers['Decimate'].iterations = 2
bpy.ops.mesh.primitive_monkey_add(location=(0, 4, 0))
bpy.data.objects['Suzanne'].modifiers.new(name='Decimate', type='DECIMATE')
bpy.data.objects['Suzanne'].modifiers['Decimate'].decimate_type = 'UNSUBDIV'
bpy.data.objects['Suzanne'].modifiers['Decimate'].iterations = 1
bpy.ops.mesh.primitive_monkey_add(location=(4, 4, 0))
bpy.data.objects['Suzanne.001'].modifiers.new(name='Decimate', type='DECIMATE')
bpy.data.objects['Suzanne.001'].modifiers['Decimate'].decimate_type = 'DISSOLVE'
bpy.data.objects['Suzanne.001'].modifiers['Decimate'].angle_limit = 0.3
bpy.ops.mesh.primitive_monkey_add(location=(-4, 4, 0))
bpy.data.objects['Suzanne.002'].modifiers.new(name='Decimate', type='DECIMATE')
bpy.data.objects['Suzanne.002'].modifiers['Decimate'].decimate_type = 'DISSOLVE'
bpy.data.objects['Suzanne.002'].modifiers['Decimate'].angle_limit = 0.6
bpy.data.objects['Suzanne.002'].modifiers['Decimate'].delimit = {'UV'}
bpy.ops.mesh.primitive_cylinder_add(location=(4, 0, 0))
bpy.data.objects['Cylinder'].modifiers.new(name='Decimate', type='DECIMATE')
bpy.data.objects['Cylinder'].modifiers['Decimate'].decimate_type = 'DISSOLVE'
bpy.ops.mesh.primitive_cylinder_add(location=(-4, 0, 0))
bpy.data.objects['Cylinder.001'].modifiers.new(name='Decimate', type='DECIMATE')
bpy.data.objects['Cylinder.001'].modifiers['Decimate'].decimate_type = 'DISSOLVE'
bpy.data.objects['Cylinder.001'].modifiers['Decimate'].angle_limit = 0.5
bpy.data.objects['Cylinder.001'].modifiers['Decimate'].use_dissolve_boundaries = True
