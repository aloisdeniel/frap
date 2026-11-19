# WP 3.8: uv.cube_project on the default cube (each face fills [0, 1]², centre = transform
# pivot), with cube_size, clip_to_bounds, and a partial selection on a moved cube.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.cube_project()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.scene.cursor.location.x = bpy.data.meshes['Cube'].uv_layers.active.data[3].uv[1]
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.uv.cube_project(cube_size=1.5, clip_to_bounds=True)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.scene.cursor.location.y = bpy.data.meshes['Cube'].uv_layers.active.data[3].uv[1]
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[1, 3, 5])
bpy.ops.transform.translate(value=(0.3, -0.2, 0.1))
bpy.ops.uv.cube_project(scale_to_bounds=True)
bpy.ops.object.mode_set(mode='OBJECT')
