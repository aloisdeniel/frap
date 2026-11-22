# WP 8.3.4: object.select_more / select_less: the parent-child pairs of which exactly one end is
# selected grow (more) or shrink (less) the selection by one level, up and down the hierarchy;
# hidden and unselectable objects are skipped; nothing to change cancels. Chain Root → Mid →
# Leaf → Tip plus Side under Mid. Each step's selection is recorded by a transform clear of the
# selected objects.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.ops.object.empty_add(location=(1, 1, 1), rotation=(0.5, 0.5, 0.5))
bpy.context.object.name = 'Root'
bpy.ops.object.empty_add(location=(1, 1, 1), rotation=(0.5, 0.5, 0.5))
bpy.context.object.name = 'Mid'
bpy.ops.object.empty_add(location=(1, 1, 1), rotation=(0.5, 0.5, 0.5))
bpy.context.object.name = 'Leaf'
bpy.ops.object.empty_add(location=(1, 1, 1), rotation=(0.5, 0.5, 0.5))
bpy.context.object.name = 'Tip'
bpy.ops.object.empty_add(location=(1, 1, 1), rotation=(0.5, 0.5, 0.5))
bpy.context.object.name = 'Side'
bpy.data.objects['Mid'].parent = bpy.data.objects['Root']
bpy.data.objects['Leaf'].parent = bpy.data.objects['Mid']
bpy.data.objects['Tip'].parent = bpy.data.objects['Leaf']
bpy.data.objects['Side'].parent = bpy.data.objects['Mid']
bpy.data.objects['Root'].scale = (2, 2, 2)
bpy.data.objects['Mid'].scale = (2, 2, 2)
bpy.data.objects['Leaf'].scale = (2, 2, 2)
bpy.data.objects['Tip'].scale = (2, 2, 2)
bpy.data.objects['Side'].scale = (2, 2, 2)
bpy.ops.object.select_all(action='DESELECT')
bpy.ops.object.select_more()
bpy.data.objects['Leaf'].select_set(True)
bpy.ops.object.select_more()
bpy.ops.object.location_clear()
bpy.ops.object.select_more()
bpy.ops.object.rotation_clear()
bpy.ops.object.select_less()
bpy.data.objects['Root'].select_set(False)
bpy.ops.object.select_less()
bpy.ops.object.scale_clear()
bpy.ops.object.select_less()
bpy.ops.transform.translate(value=(0, 0, 3))
bpy.ops.object.select_less()
bpy.ops.object.select_less()
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Mid'].select_set(True)
bpy.data.objects['Tip'].hide_set(True)
bpy.data.objects['Side'].hide_select = True
bpy.ops.object.select_more()
bpy.data.objects['Tip'].hide_set(False)
bpy.data.objects['Side'].hide_select = False
