# WP 8.3.4: object.select_camera (bl_operators/object.py): the scene camera becomes the only
# selected object and the active one; extend keeps the selection; a hidden camera becomes active
# but stays unselected; no scene camera, or one outside the scene, cancels (WARNING); the poll
# accepts Edit Mode (with extend, the camera becomes active there).
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.ops.object.select_camera()
bpy.ops.transform.translate(value=(0, 0, 1))
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.select_camera(extend=True)
bpy.ops.transform.translate(value=(0, 0, 2))
bpy.ops.object.camera_add(location=(0, -5, 0))
bpy.context.scene.camera = bpy.data.objects['Camera.001']
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.select_camera()
bpy.ops.transform.translate(value=(0, 0, 4))
bpy.data.objects['Camera'].hide_select = True
bpy.context.scene.camera = bpy.data.objects['Camera']
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.select_camera()
bpy.data.objects['Camera'].hide_select = False
bpy.context.scene.camera = None
bpy.ops.object.select_camera()
bpy.context.scene.camera = bpy.data.objects.new('Lone', bpy.data.cameras.new('Lone'))
bpy.ops.object.select_camera()
bpy.context.scene.camera = bpy.data.objects['Camera']
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.object.select_camera(extend=True)
