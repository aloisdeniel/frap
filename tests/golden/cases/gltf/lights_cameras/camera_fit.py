# WP 3.4: view3d.camera_to_view_selected moves the scene camera (rotation kept, scale
# restored) so that the selected objects' evaluated vertices fill the frame: landscape AUTO
# fit, then a portrait VERTICAL fit with lens shift, a parented camera and a modifier-free
# selection of two meshes; unselected objects and selected non-geometry objects (the light)
# are ignored.
# golden: startup=factory tier=T1 compare=objects tol=1e-4
import bpy
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=4, location=(4, 1, 0.5))
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.data.objects['Light'].select_set(True)
bpy.ops.view3d.camera_to_view_selected()
bpy.data.objects['Sphere'].select_set(True)
bpy.data.objects['Camera'].scale = (2, 1, 0.5)
bpy.data.cameras['Camera'].sensor_fit = 'VERTICAL'
bpy.data.cameras['Camera'].shift_x = 0.1
bpy.data.cameras['Camera'].shift_y = -0.2
bpy.context.scene.render.resolution_x = 800
bpy.context.scene.render.resolution_y = 1200
bpy.ops.view3d.camera_to_view_selected()
bpy.data.objects.new('Rig', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Rig'])
bpy.data.objects['Rig'].location = (1, -2, 0.5)
bpy.data.objects['Rig'].rotation_euler = (0, 0, 0.4)
bpy.data.objects['Camera'].parent = bpy.data.objects['Rig']
bpy.data.cameras['Camera'].sensor_fit = 'AUTO'
bpy.ops.view3d.camera_to_view_selected()
