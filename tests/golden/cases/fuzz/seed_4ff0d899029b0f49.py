# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Icosphere tier=T2 tol=0.0001
# fuzz: seed=0x4ff0d899029b0f49 (replay)
import bpy
bpy.ops.object.modifier_move_down(modifier='Subdivision')
bpy.context.object.modifiers['Subdivision'].show_viewport = False
bpy.context.object.modifiers['Array'].count = 3
bpy.ops.object.duplicate_move_linked(TRANSFORM_OT_translate={'value': (0, 1, 0)})
bpy.ops.object.convert(target='MESH', keep_original=False)
bpy.ops.object.parent_clear(type='CLEAR')
bpy.ops.object.vertex_group_limit_total()
bpy.ops.mesh.primitive_ico_sphere_add(location=(-0.76, -1.18, -0.28))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.object.material_slot_move(direction='UP')
bpy.ops.mesh.extrude_edges_move(TRANSFORM_OT_translate={'value': (0, 0, 1)})
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.modifier_move_to_index(modifier='Subdivision', index=2)
bpy.ops.object.modifier_apply(modifier='Subdivision')
