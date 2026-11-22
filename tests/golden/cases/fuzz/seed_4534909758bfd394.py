# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cube.001 tier=T2 tol=0.0001
# fuzz: seed=0x4534909758bfd394 (replay)
import bpy
bpy.ops.geometry.color_attribute_duplicate()
bpy.context.object.location.z = 1.04
bpy.ops.object.select_random(ratio=0.49, seed=67)
bpy.ops.object.vertex_group_copy()
bpy.ops.mesh.primitive_cube_add(location=(-1.64, -1.06, 1.26), rotation=(-0.11, -0.07, -0.22))
bpy.ops.object.select_random(seed=828)
bpy.ops.view3d.snap_selected_to_cursor()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.vert_connect()
bpy.ops.uv.select_pinned()
bpy.ops.mesh.delete(type='EDGE')
bpy.context.scene.cursor.location = (1.26, -1.89, 2.0)
bpy.context.scene.cursor.rotation_euler = (0.2, -0.89, 1.42)
bpy.context.object.vertex_groups.new(name='Group')
