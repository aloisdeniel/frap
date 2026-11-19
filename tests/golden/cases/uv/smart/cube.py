# WP 3.8: uv.smart_project() on the default cube: 6 islands in a 3x2 grid of 1/3 squares
# ([MESH App. B.5]); island_margin is remembered as the pack margin.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.smart_project()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.scene.cursor.location.x = bpy.data.meshes['Cube'].uv_layers.active.data[0].uv[0]
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.uv.smart_project(island_margin=0.05, rotate_method='AXIS_ALIGNED')
bpy.ops.object.mode_set(mode='OBJECT')
