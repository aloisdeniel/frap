# WP 1.20 (T0), object.duplicate: a child duplicated with its parent gets the parent's copy, a
# child duplicated alone keeps the original parent; object-linked slot entries are copied; an
# object in several collections of the scene (master included) is copied into all of them;
# copies of two objects sharing a mesh share one new mesh; without an active object no copy
# becomes active.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Light'].parent = bpy.data.objects['Cube']
bpy.data.objects['Cube'].location = (0, 0, 1)
bpy.data.objects['Light'].select_set(True)
bpy.data.objects['Cube'].material_slots[0].link = 'OBJECT'
bpy.context.view_layer.objects.active = None
bpy.ops.object.duplicate()
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Light'].select_set(True)
bpy.ops.object.duplicate()
bpy.data.collections.new('B')
bpy.context.scene.collection.children.link(bpy.data.collections['B'])
bpy.data.collections['B'].objects.link(bpy.data.objects['Camera'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Camera'])
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Camera'].select_set(True)
bpy.ops.object.duplicate()
bpy.data.collections['B'].objects['Camera.001']
bpy.data.collections['Collection'].objects['Camera.001']
bpy.context.scene.collection.objects['Camera.001']
bpy.data.objects.new('Twin', bpy.data.meshes['Cube'])
bpy.data.collections['Collection'].objects.link(bpy.data.objects['Twin'])
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Twin'].select_set(True)
bpy.data.objects['Cube'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Twin']
bpy.ops.object.duplicate()
bpy.data.objects['Twin.001'].data.name = 'SharedCopy'
bpy.data.objects['Cube.002'].data.name
bpy.data.meshes['Cube.003']
