# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cube.001 tier=T2 tol=0.0001
# fuzz: seed=0x63033b0ca389c35a (replay)
import bpy
bpy.ops.mesh.uv_texture_remove()
bpy.ops.object.vertex_group_clean(limit=0.5)
bpy.ops.object.shade_flat()
bpy.ops.mesh.uv_texture_add()
bpy.ops.object.align(align_mode='OPT_3')
bpy.ops.object.transform_apply()
bpy.ops.object.shape_key_lock(action='UNLOCK')
bpy.ops.mesh.uv_texture_add()
bpy.ops.object.modifier_copy(modifier='Subdivision')
bpy.ops.transform.translate(value=(-0.14, 0.53, 0.55), constraint_axis=(False, False, False), orient_type='LOCAL', use_proportional_edit=True, proportional_size=1.4, proportional_edit_falloff='ROOT')
bpy.ops.object.duplicate_move(TRANSFORM_OT_translate={'value': (1, 0, 0)})
bpy.context.view_layer.objects.active = bpy.data.objects['Camera']
bpy.ops.object.shape_key_remove(all=True, apply_mix=True)
bpy.context.scene.cursor.location = (1.62, -0.56, -0.28)
