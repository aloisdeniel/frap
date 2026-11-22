# WP 8.6: BOOLEAN modifier, solver FLOAT (BM_mesh_intersect), operation DIFFERENCE: a rotated cube
# through a corner of the default cube; T2 against Blender 5.2.4.
# golden: startup=factory compare=objects,evaluated:Cube tier=T2 tol=1e-4
import bpy
bpy.ops.mesh.primitive_cube_add(size=1.2, location=(0.7, 0.4, 0.3), rotation=(0.3, 0.2, 0.1))
bpy.data.objects['Cube'].modifiers.new(name='Boolean', type='BOOLEAN')
bpy.data.objects['Cube'].modifiers['Boolean'].object = bpy.data.objects['Cube.001']
bpy.data.objects['Cube'].modifiers['Boolean'].operation = 'DIFFERENCE'
bpy.data.objects['Cube'].modifiers['Boolean'].solver = 'FLOAT'
bpy.context.view_layer.update()
