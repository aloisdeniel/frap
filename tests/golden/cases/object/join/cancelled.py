# WP 1.20 (T0), object.join refusals: the poll fails without an active object or with a
# camera active; with only the active mesh selected it is CANCELLED ('No mesh data to join'),
# with the active mesh unselected too ('Active object is not a selected mesh'); a selected
# empty mesh adds no vertex.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.ops.object.join()
bpy.data.objects['Light'].select_set(True)
bpy.ops.object.join()
bpy.data.objects['Cube'].select_set(False)
bpy.ops.object.join()
bpy.data.objects.new('E', bpy.data.meshes.new('EM'))
bpy.data.collections['Collection'].objects.link(bpy.data.objects['E'])
bpy.data.objects['E'].select_set(True)
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.join()
bpy.context.view_layer.objects.active = bpy.data.objects['Camera']
bpy.ops.object.join.poll()
bpy.ops.object.join()
bpy.context.view_layer.objects.active = None
bpy.ops.object.join()
