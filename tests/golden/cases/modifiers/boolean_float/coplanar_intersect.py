# WP 8.6: FLOAT INTERSECT with a cutter whose three faces lie in the cube's face planes.
# golden: startup=factory compare=objects,evaluated:Cube tier=T2 tol=1e-4
import bpy
bpy.ops.mesh.primitive_cube_add(size=1, location=(0.5, 0.5, 0.5))
bpy.data.objects['Cube'].modifiers.new(name='Boolean', type='BOOLEAN')
bpy.data.objects['Cube'].modifiers['Boolean'].object = bpy.data.objects['Cube.001']
bpy.data.objects['Cube'].modifiers['Boolean'].operation = 'INTERSECT'
bpy.data.objects['Cube'].modifiers['Boolean'].solver = 'FLOAT'
bpy.context.view_layer.update()
