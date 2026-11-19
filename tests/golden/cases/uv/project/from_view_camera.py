# WP 3.8: uv.project_from_view projects through scene.camera (frap extension; Blender: the 3D
# View looking through its camera, set up on the Blender side only): camera_bounds with the
# render aspect, then without, then a shifted orthographic camera, then orthographic=True
# (along the virtual viewport, D-10).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1 tol=1e-4
import bpy
#@blender bpy.context.space_data.region_3d.view_perspective = 'CAMERA'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.project_from_view()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.scene.cursor.location.x = bpy.data.meshes['Cube'].uv_layers.active.data[0].uv[0]
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.uv.project_from_view(camera_bounds=False, scale_to_bounds=True)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.scene.cursor.location.y = bpy.data.meshes['Cube'].uv_layers.active.data[0].uv[0]
bpy.data.cameras['Camera'].type = 'ORTHO'
bpy.data.cameras['Camera'].shift_x = 0.1
bpy.context.scene.render.resolution_y = 1920
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.uv.project_from_view()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.scene.cursor.location.z = bpy.data.meshes['Cube'].uv_layers.active.data[0].uv[1]
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.uv.project_from_view(orthographic=True)
bpy.ops.object.mode_set(mode='OBJECT')
