# WP 3.8: uv.average_islands_scale rescales islands so their UV areas follow their 3D areas:
# a stretched cube's cube projection (every face [0, 1]²), uniformly, then with scale_uv and
# shear after a smart projection of a sheared box.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1 tol=1e-5
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.transform.resize(value=(2, 1, 0.5))
bpy.ops.uv.cube_project(cube_size=1)
bpy.ops.uv.average_islands_scale()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.scene.cursor.location.x = bpy.data.meshes['Cube'].uv_layers.active.data[0].uv[0]
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.uv.reset()
bpy.ops.uv.average_islands_scale(scale_uv=True, shear=True)
bpy.ops.object.mode_set(mode='OBJECT')
