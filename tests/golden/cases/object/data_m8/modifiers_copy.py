# WP 8.3.6 (T0): object.modifier_copy_to_selected copies the active object's named modifier to the
# other selected objects (unique name, settings and show flags kept, made active; a light does
# not take it: WARNING), refuses Hook (ERROR, nothing copied: RuntimeError) and an unknown name
# (CANCELLED); its poll needs another selected object with modifiers.
# object.modifiers_copy_to_selected copies the whole stack in order (the source's active
# modifier becomes active); its poll needs modifiers on the active object, of a type with
# modifiers. A single-instance type is not copied twice. Values are recorded in object
# locations and in the names of new objects.
# golden: startup=factory compare=objects tier=T0
import bpy
bpy.ops.object.modifier_copy_to_selected.poll()
bpy.ops.object.modifiers_copy_to_selected.poll()
bpy.ops.object.modifier_add(type='SUBSURF')
bpy.ops.object.modifier_add(type='BEVEL')
bpy.ops.object.modifier_add(type='CLOTH')
bpy.context.object.modifiers['Subdivision'].levels = 3
bpy.context.object.modifiers['Subdivision'].show_viewport = False
bpy.context.object.modifiers['Bevel'].width = 0.2
bpy.ops.object.modifier_copy_to_selected(modifier='Subdivision')
bpy.ops.mesh.primitive_uv_sphere_add(location=(3, 0, 0))
bpy.ops.object.modifier_add(type='SUBSURF')
bpy.ops.object.modifier_add(type='MIRROR')
bpy.data.objects['Light'].select_set(True)
bpy.data.objects['Cube'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.modifier_copy_to_selected(modifier='Subdivision')
bpy.data.objects['Sphere'].location.x = bpy.data.objects['Sphere'].modifiers['Subdivision.001'].levels
bpy.data.objects.new(bpy.data.objects['Sphere'].modifiers.active.name, None)
bpy.data.objects['Sphere'].modifiers['Subdivision.001'].show_viewport
bpy.ops.object.modifier_copy_to_selected(modifier='Nope')
bpy.ops.object.modifier_copy_to_selected(modifier='Cloth')
bpy.ops.object.modifier_copy_to_selected(modifier='Cloth')
bpy.context.object.modifiers.new('Hook', 'HOOK')
bpy.ops.object.modifier_copy_to_selected(modifier='Hook')
bpy.context.object.modifiers.remove(bpy.context.object.modifiers['Hook'])
bpy.context.object.modifiers.active = bpy.context.object.modifiers['Bevel']
bpy.ops.object.modifiers_copy_to_selected()
bpy.data.objects.new(bpy.data.objects['Sphere'].modifiers.active.name, None)
bpy.data.objects['Sphere'].location.y = bpy.data.objects['Sphere'].modifiers['Bevel'].width
bpy.data.objects['Sphere'].location.z = bpy.data.objects['Sphere'].modifiers['Subdivision.002'].levels
bpy.context.view_layer.objects.active = bpy.data.objects['Light']
bpy.ops.object.modifiers_copy_to_selected()
bpy.ops.object.modifier_copy_to_selected(modifier='Subdivision')
bpy.ops.mesh.primitive_plane_add(location=(0, 3, 0))
bpy.ops.object.modifiers_copy_to_selected()
