# WP 8.3.5 (T0), the Copy Global Transform add-on operators headless: copy_global_transform
# and copy_relative_transform finish without a visible change (blender -b has no clipboard),
# paste_transform always fails with 'Clipboard does not contain a matrix'; the polls (an
# active object or pose bone; the relative object is the scene camera).
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Cube'].location = (1, 2, 3)
bpy.ops.object.copy_global_transform()
bpy.ops.object.copy_relative_transform()
bpy.data.objects['Cube'].location = (0, 0, 0)
bpy.ops.object.paste_transform()
bpy.ops.object.paste_transform(method='BAKE', use_mirror=True, use_relative=True)
bpy.context.scene.camera = None
bpy.ops.object.copy_relative_transform()
bpy.context.view_layer.objects.active = None
bpy.ops.object.copy_global_transform()
bpy.ops.object.paste_transform()
