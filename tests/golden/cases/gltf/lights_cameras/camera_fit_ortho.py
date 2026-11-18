# WP 3.4: view3d.camera_to_view_selected with an orthographic camera: ortho_scale is fitted
# (copied into an object transform to be compared) and the camera moves across and back along
# its axis (clip_start + 1 in front of the nearest vertex), with lens shift; then a light used
# as scene camera (lens from its spot cone) and a single selected vertex (CANCELLED).
# golden: startup=factory tier=T1 compare=objects tol=1e-4
import bpy
bpy.ops.mesh.primitive_cone_add(vertices=6, location=(-2, 3, 1))
bpy.data.objects['Cube'].select_set(True)
bpy.data.cameras['Camera'].type = 'ORTHO'
bpy.data.cameras['Camera'].shift_x = 0.15
bpy.ops.view3d.camera_to_view_selected()
bpy.data.objects['Cube'].location = (bpy.data.cameras['Camera'].ortho_scale, 0, 0)
bpy.data.cameras['Camera'].ortho_scale = 2
bpy.context.scene.render.resolution_x = 500
bpy.ops.view3d.camera_to_view_selected()
bpy.data.objects['Cube'].scale = (bpy.data.cameras['Camera'].ortho_scale, 1, 1)
bpy.context.scene.camera = bpy.data.objects['Light']
bpy.ops.view3d.camera_to_view_selected()
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects.new('Dot', bpy.data.meshes.new('Dot'))
bpy.context.scene.collection.objects.link(bpy.data.objects['Dot'])
bpy.data.meshes['Dot'].from_pydata([(0, 0, 0)], [], [])
bpy.data.objects['Dot'].select_set(True)
bpy.ops.view3d.camera_to_view_selected()
