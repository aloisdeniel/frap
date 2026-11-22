# WP 8.6: FLOAT UNION with a cylinder (n-gon caps: BM_mesh_calc_tessellation_beauty's polyfill and
# beautify).
# golden: startup=factory compare=objects,evaluated:Cube tier=T2 tol=1e-4
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=10, radius=0.5, depth=3, location=(0.3, -0.2, 0), rotation=(0.4, 0, 0.2))
bpy.data.objects['Cube'].modifiers.new(name='Boolean', type='BOOLEAN')
bpy.data.objects['Cube'].modifiers['Boolean'].object = bpy.data.objects['Cylinder']
bpy.data.objects['Cube'].modifiers['Boolean'].operation = 'UNION'
bpy.data.objects['Cube'].modifiers['Boolean'].solver = 'FLOAT'
bpy.context.view_layer.update()
