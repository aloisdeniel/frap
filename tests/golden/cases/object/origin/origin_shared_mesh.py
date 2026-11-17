# WP 1.21 (T1), object.origin_set(ORIGIN_CURSOR) on two selected users of one mesh: the mesh
# moves once, both objects are offset (through their own world scale).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.data.objects.new('Cube2', bpy.data.meshes['Cube'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Cube2'])
bpy.data.objects['Cube2'].select_set(True)
bpy.data.objects['Cube2'].location = (0, 5, 0)
bpy.data.objects['Cube2'].scale = (3, 3, 3)
bpy.context.scene.cursor.location = (1, 0, 0)
bpy.ops.object.origin_set(type='ORIGIN_CURSOR')
bpy.data.meshes['Cube'].vertices[0].co = (3, 3, 3)
bpy.ops.object.origin_set(type='GEOMETRY_ORIGIN')
