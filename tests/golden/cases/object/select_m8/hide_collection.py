# WP 8.3.4: object.hide_collection(collection_index=…) isolates a layer collection (indices
# breadth first per level: 0 the scene collection, 1 Collection, 2 A, 3 B, 4 A1, 5 A2): every
# other collection is hidden, the chosen one, its ancestors and its subtree shown, and it
# becomes active; extend shows a hidden one or hides a visible one (an active collection that
# gets hidden moves to its parent); an excluded one is shown but not activated; a missing index
# cancels. After each step the visible objects are read back with select_all, the active
# collection with collection_objects_select (from the Outliner in Blender) and a new empty.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.collections.new('A')
bpy.context.scene.collection.children.link(bpy.data.collections['A'])
bpy.data.collections.new('B')
bpy.context.scene.collection.children.link(bpy.data.collections['B'])
bpy.data.collections.new('A1')
bpy.data.collections['A'].children.link(bpy.data.collections['A1'])
bpy.data.collections.new('A2')
bpy.data.collections['A'].children.link(bpy.data.collections['A2'])
bpy.data.collections['A'].objects.link(bpy.data.objects.new('oA', None))
bpy.data.collections['B'].objects.link(bpy.data.objects.new('oB', None))
bpy.data.collections['A1'].objects.link(bpy.data.objects.new('oA1', None))
bpy.data.collections['A2'].objects.link(bpy.data.objects.new('oA2', None))
bpy.ops.object.hide_collection()
bpy.ops.object.hide_collection(collection_index=99)
bpy.ops.object.hide_collection(collection_index=2)
bpy.ops.object.select_all(action='SELECT')
bpy.ops.transform.translate(value=(0, 0, 1))
bpy.ops.object.hide_collection(collection_index=4)
bpy.ops.object.select_all(action='SELECT')
bpy.ops.transform.translate(value=(0, 0, 2))
bpy.ops.object.hide_collection(collection_index=5, extend=True)
bpy.ops.object.select_all(action='DESELECT')
#@frap bpy.ops.object.collection_objects_select()
#@blender with bpy.context.temp_override(collection=bpy.context.collection): assert bpy.ops.object.collection_objects_select() == {'FINISHED'}
bpy.ops.transform.translate(value=(0, 0, 4))
bpy.ops.object.hide_collection(collection_index=5, extend=True)
bpy.ops.object.select_all(action='DESELECT')
#@frap bpy.ops.object.collection_objects_select()
#@blender with bpy.context.temp_override(collection=bpy.context.collection): assert bpy.ops.object.collection_objects_select() == {'FINISHED'}
bpy.ops.transform.translate(value=(0, 0, 8))
bpy.context.view_layer.layer_collection.children['B'].exclude = True
bpy.ops.object.hide_collection(collection_index=3)
bpy.ops.object.empty_add(location=(9, 0, 0))
bpy.ops.object.hide_collection(collection_index=1, toggle=True)
bpy.ops.object.empty_add(location=(10, 0, 0))
bpy.ops.object.hide_collection(collection_index=0)
bpy.ops.object.select_all(action='SELECT')
bpy.ops.transform.translate(value=(0, 0, 16))
