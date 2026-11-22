# golden: startup=factory compare=objects,context,mesh:Cube tier=T2 tol=0.0001
# fuzz: seed=0x7c2add9307f3487a (replay)
import bpy
bpy.ops.object.join()
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.object.name = 'Renamed'
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.add(location=(2.01, -2.57, 1.18))
bpy.ops.object.vertex_group_sort(sort_type='NAME')
bpy.ops.object.empty_add(location=(-2.2, -2.26, 1.16))
bpy.ops.object.modifier_copy(modifier='Subdivision')
bpy.ops.object.collection_add()
bpy.ops.object.select_hierarchy(direction='CHILD')
bpy.ops.object.modifier_move_up(modifier='Subdivision')
bpy.ops.object.select_camera()
bpy.ops.view3d.snap_cursor_to_center()
bpy.context.scene.cursor.rotation_euler = (1.47, 0.36, 1.87)
