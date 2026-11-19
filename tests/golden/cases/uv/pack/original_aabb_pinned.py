# WP 3.8: uv.pack_islands(udim_source='ORIGINAL_AABB') packs into the bounds of the selected
# UVs (a cube projection of size 4 spans [0.25, 0.75]); with pin=True and pin_method='LOCKED'
# an island holding a pinned UV stays in place.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1 tol=1e-4
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.cube_project(cube_size=4)
bpy.ops.uv.pack_islands(udim_source='ORIGINAL_AABB', rotate=False, shape_method='AABB')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.scene.cursor.location.x = bpy.data.meshes['Cube'].uv_layers.active.data[0].uv[0]
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.uv.smart_project(island_margin=0.02)
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[2])
bpy.ops.uv.pin()
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.pack_islands(pin=True, pin_method='LOCKED', shape_method='AABB', margin=0.1)
bpy.ops.object.mode_set(mode='OBJECT')
