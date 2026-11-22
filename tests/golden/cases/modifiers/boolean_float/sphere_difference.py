# WP 8.6: FLOAT DIFFERENCE with a UV sphere (triangle fans and quads).
# golden: startup=factory compare=objects,evaluated:Cube tier=T2 tol=1e-4
import bpy
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, radius=0.8, location=(0.9, 0.7, 0.6))
bpy.data.objects['Cube'].modifiers.new(name='Boolean', type='BOOLEAN')
bpy.data.objects['Cube'].modifiers['Boolean'].object = bpy.data.objects['Sphere']
bpy.data.objects['Cube'].modifiers['Boolean'].operation = 'DIFFERENCE'
bpy.data.objects['Cube'].modifiers['Boolean'].solver = 'FLOAT'
bpy.context.view_layer.update()
