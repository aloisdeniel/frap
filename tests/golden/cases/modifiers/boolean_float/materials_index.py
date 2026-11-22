# WP 8.6: FLOAT DIFFERENCE: the cutter's material slots are remapped by index
# (BKE_object_material_remap_calc) onto the faces it leaves behind.
# golden: startup=factory compare=objects,evaluated:Cube tier=T2 tol=1e-4
import bpy
bpy.data.materials.new(name='Red')
bpy.data.materials.new(name='Blue')
bpy.ops.mesh.primitive_cube_add(size=1.2, location=(0.7, 0.4, 0.3), rotation=(0.3, 0.2, 0.1))
bpy.data.objects['Cube.001'].data.materials.append(bpy.data.materials['Red'])
bpy.data.objects['Cube.001'].data.materials.append(bpy.data.materials['Material'])
bpy.data.objects['Cube.001'].data.polygons[1].material_index = 1
bpy.data.objects['Cube'].modifiers.new(name='Boolean', type='BOOLEAN')
bpy.data.objects['Cube'].modifiers['Boolean'].object = bpy.data.objects['Cube.001']
bpy.data.objects['Cube'].modifiers['Boolean'].operation = 'DIFFERENCE'
bpy.data.objects['Cube'].modifiers['Boolean'].solver = 'FLOAT'
bpy.context.view_layer.update()
