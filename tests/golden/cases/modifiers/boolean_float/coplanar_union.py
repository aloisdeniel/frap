# WP 8.6: FLOAT UNION of two cubes sharing a face plane (x = 1), offset in y and z.
# golden: startup=factory compare=objects,evaluated:Cube tier=T2 tol=1e-4
import bpy
bpy.ops.mesh.primitive_cube_add(size=2, location=(2, 0.5, 0.3))
bpy.data.objects['Cube'].modifiers.new(name='Boolean', type='BOOLEAN')
bpy.data.objects['Cube'].modifiers['Boolean'].object = bpy.data.objects['Cube.001']
bpy.data.objects['Cube'].modifiers['Boolean'].operation = 'UNION'
bpy.data.objects['Cube'].modifiers['Boolean'].solver = 'FLOAT'
bpy.context.view_layer.update()
