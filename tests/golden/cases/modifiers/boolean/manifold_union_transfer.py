# WP 4.6: BOOLEAN modifier, MANIFOLD UNION with material_mode TRANSFER (T2): the operand's
# material is appended to the result's slots and its faces use it; with INDEX the operand's
# faces keep their slot index.
# golden: startup=factory compare=objects,materials,evaluated:Cube tier=T2 tol=1e-4
import bpy
bpy.data.materials.new(name='Red')
bpy.data.materials.new(name='Blue')
bpy.data.objects['Cube'].data.materials.append(bpy.data.materials['Red'])
bpy.ops.mesh.primitive_cylinder_add(vertices=10, radius=0.5, depth=3, location=(0.8, 0.9, 0), rotation=(0.4, 0, 0))
bpy.data.objects['Cylinder'].data.materials.append(bpy.data.materials['Blue'])
bpy.data.objects['Cube'].modifiers.new(name='Boolean', type='BOOLEAN')
bpy.data.objects['Cube'].modifiers['Boolean'].object = bpy.data.objects['Cylinder']
bpy.data.objects['Cube'].modifiers['Boolean'].solver = 'MANIFOLD'
bpy.data.objects['Cube'].modifiers['Boolean'].operation = 'UNION'
bpy.data.objects['Cube'].modifiers['Boolean'].material_mode = 'TRANSFER'
bpy.context.view_layer.update()
