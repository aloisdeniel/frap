# WP 8.3.4: object.select_grouped (headless-extended, D-154; Blender runs a #@blender
# transcription of object_select.cc) with LIGHT_TYPE (point lights), PASS (pass_index), COLOR
# (every RGB channel within 0.005), TYPE (extend), COLLECTION (the one collection holding the
# active object; none when it is only in the scene collection), HOOK (no Hook modifier: only the
# deselect, which counts as a change). Each step's selection is recorded by a transform clear or
# a translation of the selected objects.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.ops.object.light_add(type='POINT', location=(1, 1, 1), rotation=(0.5, 0.5, 0.5))
bpy.ops.object.light_add(type='SUN', location=(1, 1, 1), rotation=(0.5, 0.5, 0.5))
bpy.ops.object.empty_add(location=(1, 1, 1), rotation=(0.5, 0.5, 0.5))
bpy.context.object.name = 'E1'
bpy.ops.object.empty_add(location=(1, 1, 1), rotation=(0.5, 0.5, 0.5))
bpy.context.object.name = 'E2'
bpy.data.objects['Cube'].location = (1, 1, 1)
bpy.data.objects['Cube'].rotation_euler = (0.5, 0.5, 0.5)
bpy.data.objects['Light'].scale = (2, 2, 2)
bpy.data.objects['Camera'].scale = (2, 2, 2)
bpy.data.objects['Cube'].scale = (2, 2, 2)
bpy.data.objects['Point'].scale = (2, 2, 2)
bpy.data.objects['Sun'].scale = (2, 2, 2)
bpy.data.objects['E1'].scale = (2, 2, 2)
bpy.data.objects['E2'].scale = (2, 2, 2)
bpy.data.objects['Cube'].pass_index = 3
bpy.data.objects['E1'].pass_index = 3
bpy.data.objects['Sun'].pass_index = 2
bpy.data.objects['Cube'].color = (1, 0, 0, 1)
bpy.data.objects['E1'].color = (1, 0.01, 0, 1)
bpy.data.objects['E2'].color = (1.004, 0.004, 0, 0.2)
bpy.data.objects['Camera'].color = (0.996, 0, 0.005, 1)
bpy.context.view_layer.objects.active = bpy.data.objects['Light']
#@frap bpy.ops.object.select_grouped(type='LIGHT_TYPE')
#@blender bpy.ops.object.select_all(action='DESELECT'); bpy.data.objects['Light'].select_set(True); bpy.data.objects['Point'].select_set(True)
bpy.ops.object.location_clear()
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
#@frap bpy.ops.object.select_grouped(type='PASS')
#@blender bpy.ops.object.select_all(action='DESELECT'); [o.select_set(True) for o in bpy.context.selectable_objects if o.pass_index == 3]
bpy.ops.object.rotation_clear()
#@frap bpy.ops.object.select_grouped(type='COLOR')
#@blender bpy.ops.object.select_all(action='DESELECT'); [o.select_set(True) for o in bpy.context.selectable_objects if all(abs(o.color[i] - bpy.data.objects['Cube'].color[i]) <= 0.005 for i in range(3))]
bpy.ops.object.scale_clear()
bpy.context.view_layer.objects.active = bpy.data.objects['Sun']
#@frap bpy.ops.object.select_grouped(type='TYPE', extend=True)
#@blender [o.select_set(True) for o in bpy.context.selectable_objects if o.type == 'LIGHT']
bpy.ops.transform.translate(value=(0, 0, 3))
bpy.data.collections['Collection'].objects.unlink(bpy.data.objects['E2'])
bpy.context.scene.collection.objects.link(bpy.data.objects['E2'])
bpy.context.view_layer.objects.active = bpy.data.objects['E1']
#@frap bpy.ops.object.select_grouped(type='COLLECTION')
#@blender bpy.ops.object.select_all(action='DESELECT'); [o.select_set(True) for o in bpy.context.selectable_objects if o.name in bpy.data.collections['Collection'].objects]
bpy.ops.transform.translate(value=(0, 0, 5))
bpy.context.view_layer.objects.active = bpy.data.objects['E2']
#@frap bpy.ops.object.select_grouped(type='COLLECTION')
#@blender bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
#@frap bpy.ops.object.select_grouped(type='HOOK')
#@blender bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Point'].select_set(True)
