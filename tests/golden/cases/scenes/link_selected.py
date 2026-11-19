# WP 3.10: an object linked into another scene, or unlinked and linked again, gets a base that
# is selected when the object's stored base flag is (`object_base_new` reads
# `ob->base_flag & BASE_SELECTED`); `select_set` on any view layer writes that flag. Operators
# in the other scene then see the object as selected.
# R1 is made last, in the window's scene: Blender evaluates only that scene (matrix_world).
# golden: startup=factory tier=T0 compare=objects,context
import bpy
bpy.data.scenes.new('S2')
bpy.data.scenes['S2'].collection.objects.link(bpy.data.objects['Cube'])
bpy.data.objects['Cube'].select_set(False)
bpy.data.objects['Light'].select_set(True)
bpy.data.scenes['S2'].collection.objects.link(bpy.data.objects['Light'])
bpy.data.collections['Collection'].objects.unlink(bpy.data.objects['Light'])
bpy.data.collections['Collection'].objects.link(bpy.data.objects['Light'])
bpy.data.scenes.new('S3')
bpy.data.objects['Cube'].select_set(True, view_layer=bpy.data.scenes['S2'].view_layers['ViewLayer'])
bpy.data.scenes['S3'].collection.objects.link(bpy.data.objects['Cube'])
bpy.data.objects['Light'].select_set(False, view_layer=bpy.data.scenes['S2'].view_layers['ViewLayer'])
bpy.data.scenes['S3'].collection.objects.link(bpy.data.objects['Light'])
bpy.context.window.scene = bpy.data.scenes['S2']
bpy.ops.object.delete()
bpy.context.window.scene = bpy.data.scenes['S3']
bpy.context.scene.collection.objects.link(bpy.data.objects.new('R1', None))
bpy.data.objects['R1'].location = (bpy.data.objects['Cube'].select_get(view_layer=bpy.data.scenes['Scene'].view_layers['ViewLayer']), bpy.data.objects['Light'].select_get(view_layer=bpy.data.scenes['Scene'].view_layers['ViewLayer']), bpy.data.objects['Cube'].select_get(view_layer=bpy.data.scenes['S2'].view_layers['ViewLayer']))
bpy.data.objects['R1'].scale = (bpy.data.objects['Light'].select_get(view_layer=bpy.data.scenes['S2'].view_layers['ViewLayer']), bpy.data.objects['Cube'].select_get(view_layer=bpy.data.scenes['S3'].view_layers['ViewLayer']), bpy.data.objects['Light'].select_get(view_layer=bpy.data.scenes['S3'].view_layers['ViewLayer']))
