# WP 8.3.4: object.add_named duplicates the object found by session_uid (when set) or name: the
# data is copied (linked=True shares it; read back by name), the copy goes to the collections
# of the original (or the active collection when the original has no enabled base), is shown
# (hide_viewport cleared), selected and active; matrix places it (its four sequences are
# columns), through its parent; an unknown or missing name is "Object not found"; Edit Mode
# fails the poll. (The original orphan is removed before the dump: Blender never evaluates its
# matrix_world, D-04.)
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Cube'].location = (1, 2, 3)
bpy.ops.object.add_named(name='Cube')
bpy.data.meshes['Cube.001']
bpy.ops.object.add_named(name='Cube', linked=True)
bpy.data.meshes['Cube.002']
bpy.ops.object.add_named(session_uid=bpy.data.objects['Light'].session_uid, name='Cube')
bpy.ops.object.add_named(name='Cube', matrix=((0, 1, 0, 0), (-1, 0, 0, 0), (0, 0, 2, 0), (5, 6, 7, 1)))
bpy.ops.object.add_named(name='Nope')
bpy.ops.object.add_named()
bpy.ops.object.empty_add(location=(0, 0, 4))
bpy.context.object.name = 'Parent'
bpy.data.objects['Camera'].parent = bpy.data.objects['Parent']
bpy.context.view_layer.update()
bpy.ops.object.add_named(name='Camera', matrix=((1, 0, 0, 0), (0, 1, 0, 0), (0, 0, 1, 0), (1, 1, 1, 1)))
bpy.data.objects.new('Orphan', bpy.data.meshes['Cube'])
bpy.data.objects['Orphan'].location = (0, -3, 0)
bpy.data.objects['Orphan'].hide_viewport = True
bpy.data.collections.new('Side')
bpy.context.scene.collection.children.link(bpy.data.collections['Side'])
bpy.context.view_layer.active_layer_collection = bpy.context.view_layer.layer_collection.children['Side']
bpy.ops.object.add_named(name='Orphan')
bpy.data.objects.remove(bpy.data.objects['Orphan'])
bpy.ops.object.add_named(name='Cube.003')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.object.add_named(name='Cube')
