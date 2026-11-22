# WP 4.6: BOOLEAN modifier, solver FLOAT (T3 bands; since WP 8.6 FLOAT is Blender's
# BM_mesh_intersect, compared at T2 in modifiers/boolean_float/).
# golden: startup=factory compare=objects,evaluated:Cube tier=T3 check=faces:8-16,verts:12-16
import bpy
bpy.ops.mesh.primitive_cube_add(size=1, location=(0.5, 0.5, 0.5))
bpy.data.objects['Cube'].modifiers.new(name='Boolean', type='BOOLEAN')
bpy.data.objects['Cube'].modifiers['Boolean'].object = bpy.data.objects['Cube.001']
bpy.data.objects['Cube'].modifiers['Boolean'].solver = 'FLOAT'
bpy.context.view_layer.update()
