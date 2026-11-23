# frap_read fuzz seed: `frap < uv_material_image.py` (from crates/frap-golden/src/fuzz/seeds/) writes uv_material_image.frap
bpy.ops.mesh.primitive_monkey_add(location=(2, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.uv.smart_project()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.object.data.uv_layers.new(name='UV2')
bpy.data.materials.new('Red')
bpy.context.object.data.materials.append(bpy.data.materials['Red'])
bpy.data.images.new('Tex', 8, 8)
bpy.data.images['Tex'].pixels[0] = 0.5
bpy.data.images['Tex'].pack()
bpy.data.images['Tex'].use_fake_user = True
bpy.ops.object.shade_smooth()
bpy.ops.wm.save_as_mainfile(filepath='uv_material_image.frap')
