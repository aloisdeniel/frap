# WP 8.3.4: object.select_same_collection selects the visible, selectable objects directly in the
# named collection (not its children), keeping the selection; an unknown name or no visible
# object passes through (frap: FINISHED, nothing changes). object.collection_objects_select
# (headless-extended, D-154: Blender needs the Outliner's collection context, given by a
# #@blender temp_override) selects the objects of the active collection and of its children.
# Each step's selection is recorded by a translation.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.collections.new('Parts')
bpy.data.collections['Collection'].children.link(bpy.data.collections['Parts'])
bpy.data.collections.new('Bolts')
bpy.data.collections['Parts'].children.link(bpy.data.collections['Bolts'])
bpy.context.view_layer.active_layer_collection = bpy.context.view_layer.layer_collection.children['Collection'].children['Parts']
bpy.ops.object.empty_add(location=(1, 0, 0))
bpy.context.object.name = 'P1'
bpy.ops.object.empty_add(location=(2, 0, 0))
bpy.context.object.name = 'P2'
bpy.context.view_layer.active_layer_collection = bpy.context.view_layer.layer_collection.children['Collection'].children['Parts'].children['Bolts']
bpy.ops.object.empty_add(location=(3, 0, 0))
bpy.context.object.name = 'B1'
bpy.data.objects['P2'].hide_select = True
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Light'].select_set(True)
bpy.ops.object.select_same_collection(collection='Parts')
bpy.ops.transform.translate(value=(0, 0, 1))
bpy.ops.object.select_all(action='DESELECT')
bpy.ops.object.select_same_collection(collection='Nope')
bpy.ops.object.select_same_collection(collection='Collection')
bpy.ops.transform.translate(value=(0, 0, 2))
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['P2'].hide_select = False
bpy.context.view_layer.active_layer_collection = bpy.context.view_layer.layer_collection.children['Collection'].children['Parts']
#@frap bpy.ops.object.collection_objects_select()
#@blender with bpy.context.temp_override(collection=bpy.context.collection): assert bpy.ops.object.collection_objects_select() == {'FINISHED'}
bpy.ops.transform.translate(value=(0, 0, 4))
bpy.ops.object.select_all(action='DESELECT')
bpy.context.view_layer.active_layer_collection = bpy.context.view_layer.layer_collection
#@frap bpy.ops.object.collection_objects_select()
#@blender with bpy.context.temp_override(collection=bpy.context.collection): assert bpy.ops.object.collection_objects_select() == {'FINISHED'}
bpy.ops.transform.translate(value=(0, 0, 8))
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['P1'].hide_viewport = True
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.select_same_collection(collection='Parts')
