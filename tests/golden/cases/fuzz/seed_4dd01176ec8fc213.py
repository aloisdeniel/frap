# golden: startup=factory compare=objects,context,mesh:Circle,mesh:Cube,mesh:Cube.001 tier=T2 tol=0.0001
# fuzz: seed=0x4dd01176ec8fc213 (replay)
import bpy
bpy.context.scene.tool_settings.transform_pivot_point = 'BOUNDING_BOX_CENTER'
bpy.ops.object.duplicate_move(TRANSFORM_OT_translate={'value': (1, 0, 0)})
bpy.ops.mesh.primitive_circle_add(location=(-0.62, -1.62, 1.85))
bpy.context.object.modifiers['Mirror'].use_bisect_axis = (True, False, False)
bpy.ops.object.vertex_group_normalize()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.normals_tools(mode='MULTIPLY')
bpy.ops.mesh.dissolve_mode(use_boundary_tear=False)
bpy.ops.mesh.set_sharpness_by_angle(extend=True)
bpy.ops.object.vertex_group_deselect()
bpy.ops.uv.select_overlap()
bpy.ops.mesh.dissolve_limited(delimit={'NORMAL', 'SHARP'})
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.select_all(action='SELECT')
