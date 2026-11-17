# M1 stabilization: the Python-only Object members `children`, `children_recursive`,
# `users_collection` and `users_scene` (tuples/lists in Blender) read as lists, index, and
# feed names into assignments; an object linked to two collections, a parent chain.
# golden: startup=factory tier=T0 compare=objects
import bpy
bpy.data.collections.new('Props')
bpy.context.scene.collection.children.link(bpy.data.collections['Props'])
bpy.data.collections['Props'].objects.link(bpy.data.objects['Cube'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Light'])
bpy.data.objects['Cube'].parent = bpy.data.objects['Camera']
bpy.data.objects['Light'].parent = bpy.data.objects['Cube']
bpy.data.objects['Cube'].users_collection
bpy.data.objects['Light'].users_collection
bpy.data.objects['Cube'].users_scene
bpy.data.objects['Camera'].children
bpy.data.objects['Camera'].children_recursive
bpy.data.objects['Light'].children
bpy.data.objects['Light'].children_recursive
print(bpy.data.objects['Cube'].users_collection)
bpy.data.collections['Props'].name = bpy.data.objects['Cube'].users_collection[1].name + 'X'
bpy.data.objects['Cube'].name = bpy.data.objects['Camera'].children[0].name + '_child'
bpy.data.objects['Light'].name = bpy.data.objects['Camera'].children_recursive[1].name + '_grandchild'
bpy.data.objects['Camera'].name = bpy.data.objects['Camera'].users_scene[0].name + '_cam'
bpy.data.objects['Scene_cam'].children[5]
