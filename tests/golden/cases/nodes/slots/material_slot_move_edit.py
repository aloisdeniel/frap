# WP 3.1: `object.material_slot_move` in Edit Mode remaps the edit mesh's face material indices
# (written back on leaving Edit Mode) and still permutes the slots and the active index.
# golden: startup=factory tier=T0 compare=objects,mesh:Cube
import bpy
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('A'))
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('B'))
bpy.data.meshes['Cube'].polygons.foreach_set('material_index', [0, 1, 2, 2, 1, 0])
bpy.data.objects['Cube'].active_material_index = 0
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.object.material_slot_move(direction='DOWN')
bpy.ops.object.material_slot_move(direction='DOWN')
bpy.ops.object.material_slot_move(direction='DOWN')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Cube'].location.x = bpy.data.objects['Cube'].active_material_index
