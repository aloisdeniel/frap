# M8 W0 (decision log M8W0-6): the glTF add-on's variant operators. Unknown while
# `KHR_materials_variants_ui` is off (`variant_register`); with it on, their polls, `VariantName`
# made unique, `assign_as_original` / `reset_to_original` (default materials),
# `assign_to_variant` (a new mapping entry, then the material replaced when the slot's entry
# already holds the active variant), `display_variant` (the slot takes the active variant's
# material), `variants_slot_add`, `material_to_variant` (by `gltf2_variant_pointer`),
# `remove_material_variant` and `variant_remove` (the entries of later variants renumbered,
# those of the removed one dropped). The variant data is copied into object names and
# transforms (compare=objects; slot materials are part of the object dump).
# golden: startup=factory tier=T0 compare=objects
import bpy
bpy.ops.scene.gltf2_variant_add()
bpy.ops.scene.gltf2_display_variant.poll()
bpy.context.preferences.addons['io_scene_gltf2'].preferences.KHR_materials_variants_ui = True
bpy.ops.scene.gltf2_display_variant()
bpy.ops.scene.gltf2_variant_remove()
bpy.ops.scene.gltf2_reset_to_original()
bpy.ops.scene.gltf2_material_to_variant()
bpy.ops.scene.gltf2_remove_material_variant()
bpy.ops.scene.gltf2_variant_add(foo=1)
bpy.data.materials.new('Red')
bpy.data.materials.new('Blue')
bpy.ops.scene.gltf2_assign_as_original()
bpy.ops.scene.gltf2_variant_add()
bpy.ops.scene.gltf2_variant_add()
bpy.data.objects['Cube'].material_slots[0].material = bpy.data.materials['Red']
bpy.ops.scene.gltf2_assign_to_variant()
bpy.data.scenes[0].gltf2_active_variant = 0
bpy.data.objects['Cube'].material_slots[0].material = bpy.data.materials['Blue']
bpy.ops.scene.gltf2_assign_to_variant()
bpy.ops.scene.gltf2_assign_to_variant()
bpy.data.objects['Cube'].material_slots[0].material = bpy.data.materials['Red']
bpy.ops.scene.gltf2_assign_to_variant()
bpy.ops.scene.gltf2_reset_to_original()
bpy.data.objects['Light'].location = (bpy.data.meshes['Cube'].gltf2_variant_mesh_data[0].variants[0].variant.variant_idx, bpy.data.meshes['Cube'].gltf2_variant_mesh_data[1].variants[0].variant.variant_idx, bpy.data.meshes['Cube'].gltf2_variant_default_materials[0].material_slot_index)
bpy.data.objects['Light'].name = bpy.data.objects['Cube'].material_slots[0].material.name + '|' + bpy.data.scenes[0].gltf2_KHR_materials_variants_variants[0].name + '|' + bpy.data.scenes[0].gltf2_KHR_materials_variants_variants[1].name + '|' + bpy.data.meshes['Cube'].gltf2_variant_mesh_data[0].material.name + '|' + bpy.data.meshes['Cube'].gltf2_variant_mesh_data[1].material.name
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[2].material
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[1].variants[1].variant
bpy.data.scenes[0].gltf2_active_variant = 1
bpy.ops.scene.gltf2_display_variant()
bpy.ops.scene.gltf2_variants_slot_add()
bpy.data.objects['Camera'].location = (bpy.data.meshes['Cube'].gltf2_variant_mesh_data[1].variants[0].variant.variant_idx, bpy.data.meshes['Cube'].gltf2_variant_mesh_data[1].variants[1].variant.variant_idx, bpy.data.meshes['Cube'].gltf2_variant_mesh_data[1].variants[2].variant.variant_idx)
bpy.ops.scene.gltf2_material_to_variant.poll()
bpy.data.meshes['Cube'].gltf2_variant_pointer = 'VariantName.001'
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[1].active_variant_idx = 1
bpy.ops.scene.gltf2_material_to_variant()
bpy.data.objects['Camera'].rotation_euler = (bpy.data.meshes['Cube'].gltf2_variant_mesh_data[1].variants[0].variant.variant_idx, bpy.data.meshes['Cube'].gltf2_variant_mesh_data[1].variants[1].variant.variant_idx, bpy.data.meshes['Cube'].gltf2_variant_mesh_data[1].variants[2].variant.variant_idx)
bpy.data.meshes['Cube'].gltf2_variant_pointer = 'Nope'
bpy.ops.scene.gltf2_material_to_variant()
bpy.ops.scene.gltf2_remove_material_variant()
bpy.data.objects['Camera'].scale = (bpy.data.meshes['Cube'].gltf2_variant_mesh_data[1].variants[0].variant.variant_idx, bpy.data.meshes['Cube'].gltf2_variant_mesh_data[1].variants[1].variant.variant_idx, 1.0)
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[1].variants[2]
bpy.data.scenes[0].gltf2_active_variant = 0
bpy.ops.scene.gltf2_variant_remove()
bpy.data.objects['Cube'].location = (bpy.data.meshes['Cube'].gltf2_variant_mesh_data[0].variants[0].variant.variant_idx, bpy.data.meshes['Cube'].gltf2_variant_mesh_data[1].variants[0].variant.variant_idx, bpy.data.scenes[0].gltf2_KHR_materials_variants_variants.find('VariantName.001'))
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[1]
bpy.ops.scene.gltf2_variant_remove()
bpy.ops.scene.gltf2_variant_remove()
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[0]
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[1]
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[0].variants[0]
bpy.ops.scene.gltf2_variant_add()
bpy.data.objects['Cube'].name = bpy.data.scenes[0].gltf2_KHR_materials_variants_variants[0].name
bpy.context.preferences.addons['io_scene_gltf2'].preferences.KHR_materials_variants_ui = False
bpy.ops.scene.gltf2_variant_remove()
bpy.ops.scene.gltf2_variant_remove.poll()
