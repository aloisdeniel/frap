# WP 8.6: FLOAT DIFFERENCE with a cutter of negative scale (is_flip: its faces are flipped before
# the boolean).
# golden: startup=factory compare=objects,evaluated:Cube tier=T2 tol=1e-4
import bpy
bpy.ops.mesh.primitive_cube_add(size=1.2, location=(0.7, 0.4, 0.3), rotation=(0.3, 0.2, 0.1), scale=(-1, 0.9, 1.1))
bpy.data.objects['Cube'].modifiers.new(name='Boolean', type='BOOLEAN')
bpy.data.objects['Cube'].modifiers['Boolean'].object = bpy.data.objects['Cube.001']
bpy.data.objects['Cube'].modifiers['Boolean'].operation = 'DIFFERENCE'
bpy.data.objects['Cube'].modifiers['Boolean'].solver = 'FLOAT'
bpy.context.view_layer.update()
