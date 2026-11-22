# M8 W0 (decision log M8W0-6): `scene.gltf2_variant_remove` walks `bpy.data.objects` and edits
# each mesh object's data, so a mesh used by two objects is renumbered twice (the add-on's
# behaviour, kept): its mapping of a later variant drops to the removed index on the first
# pass and is removed on the second. `display_variant` sets the slots of every user.
# golden: startup=factory tier=T0 compare=objects
import bpy
bpy.context.preferences.addons['io_scene_gltf2'].preferences.KHR_materials_variants_ui = True
bpy.data.materials.new('Red')
bpy.data.materials.new('Blue')
bpy.ops.object.duplicate(linked=True)
bpy.ops.scene.gltf2_variant_add()
bpy.ops.scene.gltf2_variant_add()
bpy.ops.scene.gltf2_variant_add()
bpy.data.objects['Cube'].material_slots[0].material = bpy.data.materials['Red']
bpy.ops.scene.gltf2_assign_to_variant()
bpy.data.scenes[0].gltf2_active_variant = 1
bpy.data.objects['Cube'].material_slots[0].material = bpy.data.materials['Blue']
bpy.ops.scene.gltf2_assign_to_variant()
bpy.data.objects['Cube'].material_slots[0].material = bpy.data.materials['Material']
bpy.data.scenes[0].gltf2_active_variant = 2
bpy.ops.scene.gltf2_display_variant()
bpy.data.objects['Light'].location = (bpy.data.meshes['Cube'].gltf2_variant_mesh_data[0].variants[0].variant.variant_idx, bpy.data.meshes['Cube'].gltf2_variant_mesh_data[1].variants[0].variant.variant_idx, bpy.data.scenes[0].gltf2_active_variant)
bpy.data.scenes[0].gltf2_active_variant = 1
bpy.ops.scene.gltf2_display_variant()
bpy.data.scenes[0].gltf2_active_variant = 0
bpy.ops.scene.gltf2_variant_remove()
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[0].variants[0]
bpy.data.objects['Camera'].location = (bpy.data.meshes['Cube'].gltf2_variant_mesh_data[0].material_slot_index, bpy.data.scenes[0].gltf2_KHR_materials_variants_variants.find('VariantName.002'), bpy.data.scenes[0].gltf2_active_variant)
bpy.data.objects['Camera'].name = bpy.data.meshes['Cube'].gltf2_variant_mesh_data[0].material.name
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[1]
bpy.data.objects['Cube'].active_material_index = 0
bpy.ops.scene.gltf2_reset_to_original()
bpy.ops.scene.gltf2_assign_as_original()
bpy.data.objects['Cube.001'].material_slots[0].material = bpy.data.materials['Red']
bpy.ops.scene.gltf2_reset_to_original()
