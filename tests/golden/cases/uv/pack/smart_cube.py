# WP 3.8: uv.pack_islands() after uv.smart_project on the default cube ([MESH App. B.5]):
# rotation ANY, CONCAVE shapes, the scene's uvcalc_margin (smart_project's island_margin, 0)
# since margin is not given; then with an explicit margin and AABB shapes.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1 tol=1e-4
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.smart_project()
bpy.ops.uv.pack_islands()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.scene.cursor.location.x = bpy.data.meshes['Cube'].uv_layers.active.data[0].uv[0]
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.uv.pack_islands(margin=0.05, shape_method='AABB', rotate=False)
bpy.ops.object.mode_set(mode='OBJECT')
