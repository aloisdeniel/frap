# WP 8.6: FLOAT INTERSECT with a COLLECTION operand: Blender's modifier error ("Cannot execute,
# intersect only available using exact solver") and the input mesh.
# golden: startup=factory compare=objects,evaluated:Cube tier=T2 tol=1e-4
import bpy
bpy.data.collections.new(name='Cutters')
bpy.context.scene.collection.children.link(bpy.data.collections['Cutters'])
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=1, radius=0.6, location=(1, 1, 1))
bpy.data.collections['Cutters'].objects.link(bpy.data.objects['Icosphere'])
bpy.ops.mesh.primitive_cube_add(size=0.8, location=(-1, -1, -0.8), rotation=(0.2, 0.3, 0))
bpy.data.collections['Cutters'].objects.link(bpy.data.objects['Cube.001'])
bpy.data.objects['Cube'].modifiers.new(name='Boolean', type='BOOLEAN')
bpy.data.objects['Cube'].modifiers['Boolean'].operand_type = 'COLLECTION'
bpy.data.objects['Cube'].modifiers['Boolean'].collection = bpy.data.collections['Cutters']
bpy.data.objects['Cube'].modifiers['Boolean'].operation = 'INTERSECT'
bpy.data.objects['Cube'].modifiers['Boolean'].solver = 'FLOAT'
bpy.context.view_layer.update()
