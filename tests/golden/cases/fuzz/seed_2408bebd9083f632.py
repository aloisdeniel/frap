# golden: startup=factory compare=objects,context,mesh:Cube tier=T2 tol=0.0001
# fuzz: seed=0x2408bebd9083f632 (replay)
import bpy
bpy.ops.object.vertex_group_normalize()
bpy.context.object.modifiers['Bevel'].width = 0.3
bpy.ops.object.shape_key_lock(action='UNLOCK')
bpy.ops.object.camera_add(location=(1.28, 1.91, -1.07))
bpy.ops.object.make_single_user(obdata=False)
bpy.ops.mesh.uv_texture_remove()
bpy.ops.object.origin_set(type='ORIGIN_CENTER_OF_MASS')
bpy.ops.geometry.color_attribute_add(name='Paint', data_type='BYTE_COLOR')
bpy.ops.geometry.attribute_remove()
bpy.ops.object.vertex_group_normalize_all()
bpy.ops.geometry.color_attribute_remove()
bpy.ops.object.material_slot_remove_all()
bpy.ops.object.modifier_move_up(modifier='Subdivision')
bpy.data.materials.new('Red')
