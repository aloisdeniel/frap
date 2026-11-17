# WP 1.20 (T0), object.material_slot_add / material_slot_remove: on a mesh shared by two
# objects a slot is added to the mesh, so 'Twin' gets it too, and becomes active on the Cube
# only; removing the active slot removes it for every user, shifts their object-linked entries
# and decrements the face indices above it; a new slot copies the active slot's link (OBJECT
# stays OBJECT; with another user of the mesh Blender would leave that user's slot count behind,
# which frap's derived slots cannot express, so 'Twin' is removed first).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T0
import bpy
bpy.data.objects.new('Twin', bpy.data.meshes['Cube'])
bpy.data.collections['Collection'].objects.link(bpy.data.objects['Twin'])
bpy.ops.object.material_slot_add()
bpy.ops.object.material_slot_add()
bpy.data.objects['Cube'].active_material_index
bpy.data.objects['Twin'].material_slots[2]
bpy.data.meshes['Cube'].polygons[1].material_index = 1
bpy.data.meshes['Cube'].polygons[2].material_index = 2
bpy.data.objects['Cube'].active_material_index = 1
bpy.data.objects['Cube'].material_slots[2].link = 'OBJECT'
bpy.data.objects['Cube'].material_slots[2].material = bpy.data.materials['Material']
bpy.ops.object.material_slot_remove()
bpy.data.objects['Cube'].material_slots[1].link
bpy.data.objects['Twin'].material_slots[2]
bpy.data.objects.remove(bpy.data.objects['Twin'])
bpy.ops.object.material_slot_add()
bpy.data.objects['Cube'].material_slots[2].link
bpy.data.objects['Cube'].active_material_index = 0
bpy.ops.object.material_slot_remove()
bpy.data.objects['Cube'].hide_set(True)
bpy.ops.object.material_slot_remove()
bpy.data.objects['Cube'].hide_set(False)
bpy.data.objects['Cube'].hide_viewport = True
bpy.ops.object.material_slot_add()
bpy.data.objects['Cube'].hide_viewport = False
