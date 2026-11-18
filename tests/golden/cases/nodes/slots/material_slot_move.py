# WP 3.1: `object.material_slot_move` swaps the active slot with its neighbour (UP by default,
# DOWN) in the object's entries and the shared mesh list, remaps the face material indices of
# the two slots and moves the active index; at the ends and with one slot it is CANCELLED. A
# second object sharing the mesh sees the new data order but keeps its own active index. An
# object-linked slot moves with its link. Without a `material_index` attribute it is created.
# golden: startup=factory tier=T0 compare=objects,mesh:Cube
import bpy
bpy.ops.object.material_slot_move()
bpy.data.objects.new('Other', bpy.data.meshes['Cube'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Other'])
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('A'))
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('B'))
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('C'))
bpy.data.objects['Other'].active_material_index = 2
bpy.data.objects['Cube'].active_material_index = 1
bpy.ops.object.material_slot_move()
bpy.data.objects['Cube'].location.x = bpy.data.objects['Cube'].active_material_index
bpy.data.objects['Cube'].location.y = bpy.data.objects['Other'].active_material_index
bpy.ops.object.material_slot_move(direction='UP')
bpy.data.meshes['Cube'].polygons.foreach_set('material_index', [0, 1, 2, 3, 1, 0])
bpy.ops.object.material_slot_move(direction='DOWN')
bpy.data.objects['Cube'].location.z = bpy.data.objects['Cube'].active_material_index
bpy.data.objects['Cube'].active_material_index = 3
bpy.ops.object.material_slot_move(direction='DOWN')
bpy.data.objects['Cube'].material_slots[3].link = 'OBJECT'
bpy.data.objects['Cube'].material_slots[3].material = bpy.data.materials['Material']
bpy.ops.object.material_slot_move(direction='UP')
bpy.data.objects['Cube'].scale.x = bpy.data.objects['Cube'].active_material_index
bpy.ops.object.material_slot_move(direction='UP')
bpy.data.objects['Camera'].name = bpy.data.objects['Cube'].material_slots[1].link
bpy.data.objects['Light'].name = bpy.data.objects['Other'].material_slots[1].link
