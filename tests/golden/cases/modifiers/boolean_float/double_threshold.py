# WP 8.6: FLOAT UNION with a larger double_threshold (0.01): a cutter whose face lies 0.001 above
# the cube's top face.
# golden: startup=factory compare=objects,evaluated:Cube tier=T2 tol=1e-4
import bpy
bpy.ops.mesh.primitive_cube_add(size=1, location=(0.3, 0.2, 1.501))
bpy.data.objects['Cube'].modifiers.new(name='Boolean', type='BOOLEAN')
bpy.data.objects['Cube'].modifiers['Boolean'].object = bpy.data.objects['Cube.001']
bpy.data.objects['Cube'].modifiers['Boolean'].operation = 'UNION'
bpy.data.objects['Cube'].modifiers['Boolean'].double_threshold = 0.01
bpy.data.objects['Cube'].modifiers['Boolean'].solver = 'FLOAT'
bpy.context.view_layer.update()
