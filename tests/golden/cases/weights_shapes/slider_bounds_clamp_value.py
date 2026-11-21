# golden: startup=factory tier=T1 compare=mesh:Cube,evaluated:Cube,context
# M5+M6 stabilization round 8: changing ShapeKey.slider_min / slider_max clamps the current
# value into the new range (`rna_ShapeKey_update_minmax`), also a value an F-curve wrote at
# the last frame change, and the evaluated mesh follows.
import bpy
bpy.data.objects['Cube'].shape_key_add(name='Basis')
bpy.data.objects['Cube'].shape_key_add(name='A', from_mix=False)
bpy.data.shape_keys['Key'].key_blocks['A'].data.foreach_set('co', [2, 1, 1, 2, 1, -1, 2, -1, 1, 2, -1, -1, -1, 1, 1, -1, 1, -1, -1, -1, 1, -1, -1, -1])
bpy.data.shape_keys['Key'].key_blocks['A'].slider_min = -1.0
bpy.data.shape_keys['Key'].key_blocks['A'].value = -0.5
bpy.data.shape_keys['Key'].key_blocks['A'].slider_min = -0.2
bpy.context.scene.cursor.location[0] = bpy.data.shape_keys['Key'].key_blocks['A'].value
bpy.data.shape_keys['Key'].key_blocks['A'].value = 0.9
bpy.data.shape_keys['Key'].key_blocks['A'].slider_max = 0.5
bpy.context.scene.cursor.location[1] = bpy.data.shape_keys['Key'].key_blocks['A'].value
bpy.data.shape_keys['Key'].key_blocks['A'].slider_max = 1.0
#@blender assert abs(bpy.data.shape_keys['Key'].key_blocks['A'].value - 0.5) < 1e-6
bpy.data.shape_keys['Key'].key_blocks['A'].slider_min = -1.0
bpy.data.shape_keys['Key'].key_blocks['A'].value = 0.3
bpy.data.shape_keys['Key'].key_blocks['A'].keyframe_insert('value', frame=1)
bpy.data.shape_keys['Key'].key_blocks['A'].value = -0.6
bpy.data.shape_keys['Key'].key_blocks['A'].keyframe_insert('value', frame=8)
bpy.context.scene.frame_set(4)
#@blender assert bpy.data.shape_keys['Key'].key_blocks['A'].value < -0.01
bpy.data.shape_keys['Key'].key_blocks['A'].slider_min = 0.0
bpy.context.scene.cursor.location[2] = bpy.data.shape_keys['Key'].key_blocks['A'].value - 1.0
