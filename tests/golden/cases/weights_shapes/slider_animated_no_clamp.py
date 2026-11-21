# golden: startup=factory tier=T1 compare=mesh:Cube,evaluated:Cube,context
# M5+M6 stabilization round 9: an animated ShapeKey.slider_min is written through the setter
# only (`BKE_animsys_write_to_rna_path` never calls `rna_ShapeKey_update_minmax`), so the
# block's value stays outside the slider range, the evaluated mesh uses it, and the value is
# still the user's after the slider comes back.
import bpy
bpy.data.objects['Cube'].shape_key_add(name='Basis')
bpy.data.objects['Cube'].shape_key_add(name='A', from_mix=False)
bpy.data.shape_keys['Key'].key_blocks['A'].data.foreach_set('co', [3, 3, 3, 3, 3, -1, 3, -1, 3, 3, -1, -1, -1, 3, 3, -1, 3, -1, -1, -1, 3, -1, -1, -1])
bpy.data.shape_keys['Key'].key_blocks['A'].slider_min = -1.0
bpy.data.shape_keys['Key'].key_blocks['A'].keyframe_insert('slider_min', frame=1)
bpy.data.shape_keys['Key'].key_blocks['A'].slider_min = 0.5
bpy.data.shape_keys['Key'].key_blocks['A'].keyframe_insert('slider_min', frame=10)
bpy.data.shape_keys['Key'].key_blocks['A'].slider_min = -1.0
bpy.data.shape_keys['Key'].key_blocks['A'].value = 0.1
bpy.context.scene.frame_set(10)
#@blender assert abs(bpy.data.shape_keys['Key'].key_blocks['A'].slider_min - 0.5) < 1e-6
#@blender assert abs(bpy.data.shape_keys['Key'].key_blocks['A'].value - 0.1) < 1e-6
bpy.context.scene.cursor.location[0] = bpy.data.shape_keys['Key'].key_blocks['A'].value
bpy.context.scene.cursor.location[1] = bpy.data.shape_keys['Key'].key_blocks['A'].slider_min
bpy.context.scene.frame_set(1)
bpy.context.scene.cursor.location[2] = bpy.data.shape_keys['Key'].key_blocks['A'].value
