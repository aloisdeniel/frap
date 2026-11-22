# WP 8.6: FLOAT DIFFERENCE where the object itself is moved, rotated and scaled (the operand is
# brought into the object's space).
# golden: startup=factory compare=objects,evaluated:Cube tier=T2 tol=1e-4
import bpy
bpy.data.objects['Cube'].location = (0.3, -0.2, 0.1)
bpy.data.objects['Cube'].rotation_euler = (0.1, 0.4, 0.2)
bpy.data.objects['Cube'].scale = (1.2, 0.8, 1.0)
bpy.ops.mesh.primitive_cube_add(size=1.2, location=(0.7, 0.4, 0.3), rotation=(0.3, 0.2, 0.1))
bpy.data.objects['Cube'].modifiers.new(name='Boolean', type='BOOLEAN')
bpy.data.objects['Cube'].modifiers['Boolean'].object = bpy.data.objects['Cube.001']
bpy.data.objects['Cube'].modifiers['Boolean'].operation = 'DIFFERENCE'
bpy.data.objects['Cube'].modifiers['Boolean'].solver = 'FLOAT'
bpy.context.view_layer.update()
