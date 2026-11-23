# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Sphere tier=T2 tol=0.0001
# fuzz: seed=0x89ab259a9db6c2c5 (replay)
import bpy
bpy.ops.view3d.snap_cursor_to_grid()
bpy.ops.object.parent_clear()
bpy.ops.object.modifier_move_down(modifier='Subdivision')
bpy.context.object.vertex_groups.new(name='Weights')
bpy.ops.object.add(type='EMPTY', location=(-0.24, 2.14, 0.42))
bpy.context.object.data.polygons[2].use_smooth = False
bpy.ops.object.transform_apply(rotation=True, scale=True)
bpy.ops.object.visual_transform_apply()
bpy.ops.object.modifier_apply(modifier='Subdivision')
bpy.ops.object.visual_transform_apply()
bpy.ops.object.material_slot_add()
bpy.ops.transform.translate(value=(0.39, -0.22, 0.02), orient_type='GLOBAL', proportional_size=1.67)
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, location=(0.01, 1.97, -1.59))
bpy.ops.mesh.uv_texture_add()
