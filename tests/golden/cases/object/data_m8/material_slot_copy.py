# WP 8.3.6 (T0): object.material_slot_copy assigns the active object's slot materials (an
# OBJECT slot's own material) to the other selected editable meshes: a target with more slots
# loses its active slot until the counts match, one with none grows data slots; a linked
# duplicate (same mesh) only takes the OBJECT slots, inheriting their link; a target with the
# same slot count takes the active slot. Its poll needs an active object with a slot. Values
# are recorded in object locations and in the names of new objects (the slot links).
# golden: startup=factory compare=objects tier=T0
import bpy
bpy.ops.object.material_slot_copy.poll()
bpy.data.materials.new('Red')
bpy.data.materials.new('Blue')
bpy.data.materials.new('Green')
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.ops.object.material_slot_add()
bpy.ops.object.material_slot_add()
bpy.ops.object.material_slot_add()
bpy.context.object.material_slots[0].material = bpy.data.materials['Green']
bpy.context.object.material_slots[2].material = bpy.data.materials['Red']
bpy.context.object.active_material_index = 0
bpy.ops.mesh.primitive_uv_sphere_add(location=(0, 3, 0))
bpy.ops.object.duplicate(linked=True)
bpy.context.object.name = 'Twin'
bpy.ops.object.select_all(action='DESELECT')
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.material_slot_copy.poll()
bpy.ops.object.material_slot_add()
bpy.context.object.material_slots[1].link = 'OBJECT'
bpy.context.object.material_slots[1].material = bpy.data.materials['Blue']
bpy.context.object.active_material_index = 1
bpy.data.objects['Plane'].select_set(True)
bpy.data.objects['Sphere'].select_set(True)
bpy.data.objects['Camera'].select_set(True)
bpy.ops.object.material_slot_copy()
bpy.data.objects['Plane'].location.z = bpy.data.objects['Plane'].active_material_index
bpy.data.objects.new(bpy.data.objects['Plane'].material_slots[0].link, None)
bpy.data.objects.new(bpy.data.objects['Plane'].material_slots[1].link, None)
bpy.data.objects['Sphere'].location.z = bpy.data.objects['Sphere'].active_material_index
bpy.data.objects.new(bpy.data.objects['Sphere'].material_slots[0].link, None)
bpy.data.objects.new(bpy.data.objects['Sphere'].material_slots[1].link, None)
bpy.ops.object.select_all(action='DESELECT')
bpy.context.view_layer.objects.active = bpy.data.objects['Sphere']
bpy.context.object.material_slots[0].link = 'OBJECT'
bpy.context.object.material_slots[0].material = bpy.data.materials['Green']
bpy.context.object.material_slots[1].material = bpy.data.materials['Red']
bpy.data.objects['Twin'].select_set(True)
bpy.ops.object.material_slot_copy()
bpy.data.objects.new(bpy.data.objects['Twin'].material_slots[0].link, None)
bpy.data.objects.new(bpy.data.objects['Twin'].material_slots[1].link, None)
bpy.data.objects['Twin'].location.z = bpy.data.objects['Twin'].active_material_index
