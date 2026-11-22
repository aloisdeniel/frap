# WP 8.6: FLOAT DIFFERENCE with a 64x32 UV sphere: above 1024 triangles, so the BVH overlap
# runs Blender's threaded traversal order (one stack per root child, newest first). Blender's
# vertex order varies between runs (D-145), the canonical (T2) result does not.
# golden: startup=factory compare=objects,evaluated:Cube tier=T2 tol=1e-4
import bpy
bpy.ops.mesh.primitive_uv_sphere_add(segments=64, ring_count=32, radius=0.8, location=(0.9, 0.7, 0.6))
bpy.data.objects['Cube'].modifiers.new(name='Boolean', type='BOOLEAN')
bpy.data.objects['Cube'].modifiers['Boolean'].object = bpy.data.objects['Sphere']
bpy.data.objects['Cube'].modifiers['Boolean'].operation = 'DIFFERENCE'
bpy.data.objects['Cube'].modifiers['Boolean'].solver = 'FLOAT'
bpy.context.view_layer.update()
