# WP 1.21 (T0), object.origin_set on an Empty and a camera (nothing changes, FINISHED) and with
# nothing selected (CANCELLED); an empty mesh does not move.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects.new('Empty', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Empty'])
bpy.data.objects['Empty'].location = (1, 1, 1)
bpy.data.objects['Cube'].select_set(False)
bpy.data.objects['Empty'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Empty']
bpy.data.objects['Camera'].select_set(True)
bpy.context.scene.cursor.location = (1, 2, 3)
bpy.ops.object.origin_set(type='ORIGIN_CURSOR')
bpy.ops.object.select_all(action='DESELECT')
bpy.ops.object.origin_set(type='ORIGIN_CURSOR')
bpy.data.meshes.new('Nothing')
bpy.data.objects.new('E', bpy.data.meshes['Nothing'])
bpy.context.scene.collection.objects.link(bpy.data.objects['E'])
bpy.data.objects['E'].select_set(True)
bpy.data.objects['E'].location = (1, 1, 1)
bpy.ops.object.origin_set(type='ORIGIN_GEOMETRY')
bpy.ops.object.origin_set(type='ORIGIN_CENTER_OF_MASS')
bpy.ops.object.origin_set(type='ORIGIN_CENTER_OF_VOLUME')
