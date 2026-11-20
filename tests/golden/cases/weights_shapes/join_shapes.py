# WP 6.3: `object.join_shapes` (selected meshes with the active mesh's vertex count become
# blocks named after the objects, from their evaluated meshes, in their own local space; a
# Basis is created first; `use_mirror` swaps .L/.R and mirrors along X) and
# `object.modifier_apply_as_shapekey` (the modifier alone on the key mix, as a new block; the
# modifier is kept with `keep_modifier`; constructive modifiers refused). Two linked duplicates
# of the target show single blocks.
# golden: startup=factory tier=T1 compare=objects,mesh:Cube,evaluated:Cube,evaluated:Cube.001,evaluated:Cube.002,evaluated:Src.R
import bpy
bpy.ops.object.join_shapes()
bpy.ops.object.duplicate(linked=True)
bpy.ops.object.duplicate(linked=True)
bpy.ops.object.duplicate()
bpy.data.objects['Cube.003'].name = 'Src.R'
bpy.ops.object.duplicate()
bpy.data.objects['Src.R.001'].name = 'Arm.L'
bpy.data.objects['Src.R'].location = (5, 0, 0)
bpy.data.objects['Src.R'].modifiers.new('Cast', 'CAST')
bpy.data.objects['Src.R'].modifiers['Cast'].factor = 1.0
bpy.data.objects['Arm.L'].scale = (2, 2, 2)
bpy.data.meshes['Cube.002'].vertices[0].co = (1.5, 1.5, 1.5)
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Src.R'].select_set(True)
bpy.data.objects['Arm.L'].select_set(True)
bpy.data.objects['Cube'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.join_shapes()
bpy.data.meshes['Cube'].shape_keys.key_blocks.keys()
#@blender assert bpy.data.meshes['Cube'].shape_keys.key_blocks.keys() == ['Basis', 'Src.R', 'Arm.L']
bpy.ops.object.join_shapes(use_mirror=True)
bpy.data.meshes['Cube'].shape_keys.key_blocks.keys()
#@blender assert bpy.data.meshes['Cube'].shape_keys.key_blocks.keys() == ['Basis', 'Src.R', 'Arm.L', 'Src.L', 'Arm.R']
bpy.data.objects['Camera'].select_set(True)
bpy.data.objects['Camera'].name = 'Plain'
bpy.ops.object.join_shapes(use_mirror=True)
bpy.data.objects['Cube.001'].select_set(True)
bpy.ops.object.join_shapes(use_mirror=True)
bpy.data.objects['Cube.001'].select_set(False)
bpy.data.objects['Src.R'].modifiers.new('Weld', 'WELD')
bpy.data.objects['Src.R'].modifiers['Weld'].merge_threshold = 10.0
bpy.ops.object.join_shapes()
bpy.data.objects['Src.R'].modifiers.remove(bpy.data.objects['Src.R'].modifiers['Weld'])
bpy.data.meshes['Cube'].shape_keys.key_blocks['Src.L'].value = 1.0
bpy.data.objects['Cube'].modifiers.new('Twist', 'SIMPLE_DEFORM')
bpy.data.objects['Cube'].modifiers['Twist'].deform_method = 'BEND'
bpy.data.objects['Cube'].modifiers['Twist'].angle = 0.5
bpy.data.objects['Cube'].modifiers.new('Sub', 'SUBSURF')
bpy.ops.object.modifier_apply_as_shapekey(modifier='Sub')
bpy.ops.object.modifier_apply_as_shapekey(modifier='Nope')
bpy.ops.object.modifier_apply_as_shapekey(modifier='Twist', keep_modifier=True)
bpy.data.objects['Cube'].modifiers.keys()
bpy.ops.object.modifier_apply_as_shapekey(modifier='Twist')
bpy.data.objects['Cube'].modifiers.keys()
bpy.data.meshes['Cube'].shape_keys.key_blocks.keys()
bpy.data.objects['Cube'].modifiers.remove(bpy.data.objects['Cube'].modifiers['Sub'])
bpy.data.meshes['Cube'].shape_keys.key_blocks['Twist.001'].value = 1.0
bpy.data.meshes['Cube'].shape_keys.key_blocks['Src.L'].value = 0.0
bpy.context.view_layer.objects.active = bpy.data.objects['Src.R']
bpy.ops.object.modifier_apply_as_shapekey(modifier='Cast')
bpy.data.meshes['Cube.001'].shape_keys.key_blocks.keys()
bpy.data.objects['Cube.001'].show_only_shape_key = True
bpy.data.objects['Cube.001'].active_shape_key_index = 3
bpy.data.objects['Cube.002'].show_only_shape_key = True
bpy.data.objects['Cube.002'].active_shape_key_index = 4
