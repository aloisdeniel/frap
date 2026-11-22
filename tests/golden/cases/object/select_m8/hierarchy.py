# WP 8.3.4: object.select_hierarchy (bl_operators/object.py): CHILD selects the visible
# children of the selected objects and of the active one, the first by name becomes active;
# PARENT selects their visible parents, the active object's parent becomes active (none → no
# active object, after which the poll fails); extend keeps the selection; no visible candidate
# cancels; a hidden parent is skipped. Each step's selection is recorded by a transform clear of
# the selected objects.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.ops.object.empty_add(location=(1, 1, 1), rotation=(0.5, 0.5, 0.5))
bpy.context.object.name = 'b'
bpy.ops.object.empty_add(location=(1, 1, 1), rotation=(0.5, 0.5, 0.5))
bpy.context.object.name = 'A'
bpy.ops.object.empty_add(location=(1, 1, 1), rotation=(0.5, 0.5, 0.5))
bpy.context.object.name = 'A2'
bpy.ops.object.empty_add(location=(1, 1, 1), rotation=(0.5, 0.5, 0.5))
bpy.context.object.name = 'A1'
bpy.data.objects['A'].parent = bpy.data.objects['Cube']
bpy.data.objects['b'].parent = bpy.data.objects['Cube']
bpy.data.objects['A1'].parent = bpy.data.objects['A']
bpy.data.objects['A2'].parent = bpy.data.objects['A']
bpy.data.objects['Cube'].location = (1, 1, 1)
bpy.data.objects['Cube'].rotation_euler = (0.5, 0.5, 0.5)
bpy.data.objects['Cube'].scale = (2, 2, 2)
bpy.data.objects['A'].scale = (2, 2, 2)
bpy.data.objects['b'].scale = (2, 2, 2)
bpy.data.objects['A1'].scale = (2, 2, 2)
bpy.data.objects['A2'].scale = (2, 2, 2)
bpy.ops.object.select_all(action='DESELECT')
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.select_hierarchy(direction='CHILD')
bpy.ops.object.location_clear()
bpy.ops.object.select_hierarchy(direction='CHILD')
bpy.ops.object.rotation_clear()
bpy.ops.object.select_hierarchy(direction='PARENT')
bpy.ops.object.scale_clear()
bpy.ops.object.select_hierarchy(direction='PARENT', extend=True)
bpy.ops.object.select_hierarchy(direction='PARENT')
bpy.ops.object.select_hierarchy(direction='PARENT')
bpy.context.view_layer.objects.active = bpy.data.objects['A2']
bpy.data.objects['A'].hide_set(True)
bpy.ops.object.select_hierarchy(direction='PARENT')
bpy.ops.object.select_hierarchy(direction='CHILD')
bpy.data.objects['A'].hide_set(False)
bpy.ops.object.select_hierarchy(direction='PARENT', extend=True)
bpy.ops.object.select_hierarchy(direction='UP')
