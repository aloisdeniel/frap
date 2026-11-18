# WP 2.9: object.material_slot_assign assigns the active slot to the selected faces in Edit
# Mode (material_index is written on leaving Edit Mode); in Object Mode, and with no face
# selected, nothing is assigned (CANCELLED).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.material_slot_add()
bpy.ops.object.material_slot_add()
bpy.data.objects['Cube'].material_slots[1].material = bpy.data.materials['Material']
bpy.ops.object.material_slot_assign()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.object.material_slot_assign()
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[1, 4])
bpy.data.objects['Cube'].active_material_index = 2
bpy.ops.object.material_slot_assign()
bpy.ops.frap.select_elements(type='FACE', indices=[2])
bpy.data.objects['Cube'].active_material_index = 1
bpy.ops.object.material_slot_assign()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.scene.cursor.location.x = bpy.data.meshes['Cube'].polygons[2].material_index
bpy.context.scene.cursor.location.y = bpy.data.meshes['Cube'].polygons[1].material_index
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.data.objects['Cube'].active_material_index = 0
bpy.ops.object.material_slot_assign()
