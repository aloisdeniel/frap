# WP 8.6: FLOAT DIFFERENCE with an open operand (a tilted plane through the cube): the FLOAT
# solver cuts with open surfaces (the EXACT stand-in refuses them).
# golden: startup=factory compare=objects,evaluated:Cube tier=T2 tol=1e-4
import bpy
bpy.ops.mesh.primitive_plane_add(size=3, location=(0, 0, 0.2), rotation=(0.3, 0.1, 0))
bpy.data.objects['Cube'].modifiers.new(name='Boolean', type='BOOLEAN')
bpy.data.objects['Cube'].modifiers['Boolean'].object = bpy.data.objects['Plane']
bpy.data.objects['Cube'].modifiers['Boolean'].operation = 'DIFFERENCE'
bpy.data.objects['Cube'].modifiers['Boolean'].solver = 'FLOAT'
bpy.context.view_layer.update()
