# frap_read fuzz seed: `frap < modifiers_shapekeys.py` (from crates/frap-golden/src/fuzz/seeds/) writes modifiers_shapekeys.frap
bpy.ops.object.modifier_add(type='SUBSURF')
bpy.ops.object.modifier_add(type='BEVEL')
bpy.ops.object.modifier_add(type='ARRAY')
bpy.ops.object.modifier_add(type='MIRROR')
bpy.context.object.modifiers['Array'].count = 3
bpy.context.object.vertex_groups.new(name='Group')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.object.vertex_group_assign()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.object.shape_key_add(name='Basis')
bpy.context.object.shape_key_add(name='Key')
bpy.data.shape_keys['Key'].key_blocks['Key'].value = 0.5
bpy.data.shape_keys['Key'].key_blocks['Key'].data[0].co = (1.5, 1.5, 1.5)
bpy.ops.wm.save_as_mainfile(filepath='modifiers_shapekeys.frap')
