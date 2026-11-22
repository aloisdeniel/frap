# WP 8.6: FLOAT UNION where the object's own mesh is open (the cube without its top face) and the
# cutter is a sphere poking through the opening.
# golden: startup=factory compare=objects,evaluated:Cube tier=T2 tol=1e-4
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[5])
bpy.ops.mesh.delete(type='FACE')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, radius=0.7, location=(0.2, 0.1, 1.15))
bpy.data.objects['Cube'].modifiers.new(name='Boolean', type='BOOLEAN')
bpy.data.objects['Cube'].modifiers['Boolean'].object = bpy.data.objects['Sphere']
bpy.data.objects['Cube'].modifiers['Boolean'].operation = 'UNION'
bpy.data.objects['Cube'].modifiers['Boolean'].solver = 'FLOAT'
bpy.context.view_layer.update()
