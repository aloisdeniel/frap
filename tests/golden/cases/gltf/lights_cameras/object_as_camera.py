# WP 3.4: view3d.object_as_camera makes the active object (any type) the scene camera;
# camera_to_view_selected without a scene camera is `RuntimeError: Error: No active camera`;
# with nothing selected it is CANCELLED.
# golden: startup=factory tier=T0 compare=objects,context
import bpy
bpy.ops.object.camera_add(location=(0, -8, 2), rotation=(1.3, 0, 0))
bpy.context.scene.camera = None
bpy.ops.view3d.camera_to_view_selected()
bpy.ops.view3d.object_as_camera()
bpy.data.objects['Cube'].location = bpy.context.scene.camera.location
bpy.ops.object.select_all(action='DESELECT')
bpy.ops.view3d.camera_to_view_selected()
bpy.context.view_layer.objects.active = bpy.data.objects['Light']
bpy.ops.view3d.object_as_camera()
bpy.data.objects['Cube'].location = bpy.context.scene.camera.location
bpy.context.view_layer.objects.active = None
bpy.ops.view3d.object_as_camera()
bpy.data.objects['Cube'].location = bpy.context.scene.camera.location
