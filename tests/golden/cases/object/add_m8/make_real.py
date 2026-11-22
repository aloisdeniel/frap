# WP 8.3.4: object.duplicates_make_real on collection instances: every object of the instanced
# collection and its children (an object hidden in the viewport, or reached only through a
# hidden collection, is skipped; a nested instance is followed) becomes a copy at the instancer's
# matrix times the collection's instance_offset times the object's matrix, without parent
# (default), with the copied parents (use_hierarchy) and the instancer as parent of the rest
# (use_base_parent); a modifier's object pointing into the collection points to the copy (the
# evaluated ARRAY of K.001 follows E.001); the
# instancer stops instancing and is deselected, the copies are selected; the active object
# stays; a selected object that instances nothing is left alone.
# golden: startup=factory compare=objects,context,evaluated:K.001 tier=T0
import bpy
bpy.data.collections.new('Src')
bpy.data.collections['Src'].instance_offset = (1, 0, 0)
bpy.data.collections.new('Sub')
bpy.data.collections['Src'].children.link(bpy.data.collections['Sub'])
bpy.data.collections.new('Off')
bpy.data.collections['Src'].children.link(bpy.data.collections['Off'])
bpy.data.collections['Off'].hide_viewport = True
bpy.data.collections.new('Leaf')
bpy.data.collections['Src'].objects.link(bpy.data.objects.new('E', None))
bpy.data.objects['E'].location = (0, 2, 0)
bpy.data.objects['E'].rotation_euler = (0, 0, 0.5)
bpy.data.collections['Src'].objects.link(bpy.data.objects.new('K', bpy.data.meshes['Cube']))
bpy.data.objects['K'].parent = bpy.data.objects['E']
bpy.data.objects['K'].location = (0, 0, 1)
bpy.data.objects['K'].scale = (0.5, 0.5, 0.5)
bpy.data.objects['K'].modifiers.new('Arr', 'ARRAY')
bpy.data.objects['K'].modifiers['Arr'].use_relative_offset = False
bpy.data.objects['K'].modifiers['Arr'].use_object_offset = True
bpy.data.objects['K'].modifiers['Arr'].offset_object = bpy.data.objects['E']
bpy.data.collections['Sub'].objects.link(bpy.data.objects.new('S', None))
bpy.data.objects['S'].location = (0, 0, -1)
bpy.data.collections['Sub'].objects.link(bpy.data.objects.new('Ghost', None))
bpy.data.objects['Ghost'].hide_viewport = True
bpy.data.collections['Off'].objects.link(bpy.data.objects.new('Dark', None))
bpy.data.collections['Leaf'].objects.link(bpy.data.objects.new('L', None))
bpy.data.objects['L'].location = (0, 0, 7)
bpy.data.collections['Sub'].objects.link(bpy.data.objects.new('Nest', None))
bpy.data.objects['Nest'].instance_type = 'COLLECTION'
bpy.data.objects['Nest'].instance_collection = bpy.data.collections['Leaf']
bpy.data.objects['Nest'].location = (3, 0, 0)
bpy.ops.object.collection_instance_add(name='Src', location=(5, 0, 0), rotation=(0, 0, 1.5707963), scale=(2, 2, 2))
bpy.ops.object.collection_instance_add(name='Src', location=(0, -5, 0))
bpy.ops.object.collection_instance_add(name='Src', location=(0, 5, 0))
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Src'].select_set(True)
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.duplicates_make_real()
bpy.data.objects['Src'].instance_collection.name
bpy.ops.transform.translate(value=(0, 0, 100))
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Src.001'].select_set(True)
bpy.ops.object.duplicates_make_real(use_base_parent=True, use_hierarchy=True)
bpy.ops.object.location_clear()
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Src.002'].select_set(True)
bpy.ops.object.duplicates_make_real(use_base_parent=True)
bpy.ops.object.duplicates_make_real(use_hierarchy=True)
