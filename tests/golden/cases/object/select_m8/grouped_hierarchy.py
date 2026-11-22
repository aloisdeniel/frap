# WP 8.3.4: object.select_grouped (headless-extended, D-154: Blender's poll refuses background
# mode, so Blender runs a #@blender transcription of object_select.cc) with CHILDREN_RECURSIVE,
# CHILDREN, PARENT (extend: the parent becomes active) and SIBLINGS, on Cube → (A → (A1 → A11,
# A2), B). Each step's selection is recorded by a transform clear of the selected objects
# (location, rotation, scale); a hidden child (A2) and an unselectable one (B) are skipped.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.ops.object.empty_add(location=(1, 1, 1), rotation=(0.5, 0.5, 0.5))
bpy.context.object.name = 'A'
bpy.ops.object.empty_add(location=(1, 1, 1), rotation=(0.5, 0.5, 0.5))
bpy.context.object.name = 'B'
bpy.ops.object.empty_add(location=(1, 1, 1), rotation=(0.5, 0.5, 0.5))
bpy.context.object.name = 'A1'
bpy.ops.object.empty_add(location=(1, 1, 1), rotation=(0.5, 0.5, 0.5))
bpy.context.object.name = 'A2'
bpy.ops.object.empty_add(location=(1, 1, 1), rotation=(0.5, 0.5, 0.5))
bpy.context.object.name = 'A11'
bpy.data.objects['A'].parent = bpy.data.objects['Cube']
bpy.data.objects['B'].parent = bpy.data.objects['Cube']
bpy.data.objects['A1'].parent = bpy.data.objects['A']
bpy.data.objects['A2'].parent = bpy.data.objects['A']
bpy.data.objects['A11'].parent = bpy.data.objects['A1']
bpy.data.objects['Cube'].location = (1, 1, 1)
bpy.data.objects['Cube'].rotation_euler = (0.5, 0.5, 0.5)
bpy.data.objects['Light'].scale = (2, 2, 2)
bpy.data.objects['Camera'].scale = (2, 2, 2)
bpy.data.objects['Cube'].scale = (2, 2, 2)
bpy.data.objects['A'].scale = (2, 2, 2)
bpy.data.objects['B'].scale = (2, 2, 2)
bpy.data.objects['A1'].scale = (2, 2, 2)
bpy.data.objects['A2'].scale = (2, 2, 2)
bpy.data.objects['A11'].scale = (2, 2, 2)
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Light'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
#@frap bpy.ops.object.select_grouped(type='CHILDREN_RECURSIVE')
#@blender bpy.ops.object.select_all(action='DESELECT'); [o.select_set(True) for o in bpy.context.selectable_objects if o in bpy.data.objects['Cube'].children_recursive]
bpy.ops.object.location_clear()
bpy.context.view_layer.objects.active = bpy.data.objects['A']
bpy.data.objects['Camera'].select_set(True)
#@frap bpy.ops.object.select_grouped(type='CHILDREN')
#@blender bpy.ops.object.select_all(action='DESELECT'); [o.select_set(True) for o in bpy.context.selectable_objects if o.parent == bpy.data.objects['A']]
bpy.ops.object.rotation_clear()
bpy.context.view_layer.objects.active = bpy.data.objects['A1']
#@frap bpy.ops.object.select_grouped(type='PARENT', extend=True)
#@blender bpy.data.objects['A'].select_set(True); bpy.context.view_layer.objects.active = bpy.data.objects['A']
bpy.ops.object.scale_clear()
bpy.data.objects['A2'].hide_set(True)
bpy.data.objects['B'].hide_select = True
#@frap bpy.ops.object.select_grouped(type='CHILDREN')
#@blender bpy.ops.object.select_all(action='DESELECT'); bpy.data.objects['A1'].select_set(True)
bpy.data.objects['B'].hide_select = False
bpy.context.view_layer.objects.active = bpy.data.objects['B']
#@frap bpy.ops.object.select_grouped(type='SIBLINGS', extend=True)
#@blender bpy.data.objects['A'].select_set(True); bpy.data.objects['B'].select_set(True)
