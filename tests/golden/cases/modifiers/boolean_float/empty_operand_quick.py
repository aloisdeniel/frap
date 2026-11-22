# WP 8.6: FLOAT with an operand without faces (get_quick_mesh): DIFFERENCE keeps the mesh.
# golden: startup=factory compare=objects,evaluated:Cube tier=T2 tol=1e-4
import bpy
bpy.ops.mesh.primitive_plane_add(size=1, location=(0, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.delete(type='ONLY_FACE')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Cube'].modifiers.new(name='Boolean', type='BOOLEAN')
bpy.data.objects['Cube'].modifiers['Boolean'].object = bpy.data.objects['Plane']
bpy.data.objects['Cube'].modifiers['Boolean'].operation = 'DIFFERENCE'
bpy.data.objects['Cube'].modifiers['Boolean'].solver = 'FLOAT'
bpy.context.view_layer.update()
