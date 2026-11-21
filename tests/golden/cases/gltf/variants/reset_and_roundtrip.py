# WP 7.5: `variants_reset_to_original` and the import. With default materials recorded, the
# export first writes them back into the material slots of the exported mesh objects (and the
# slots keep them afterwards: the cube shows Red again). The file is imported into an empty
# file: the variant list goes to `bpy.data.scenes[0]` with the preference turned on, every
# primitive with mappings gets its own slot (not merged), a default-material entry and one
# entry per mapping with its variant pointers. The import is exported again (same mappings).
# golden: startup=factory tier=T2 compare=objects,glb:out/a.glb,glb:out/b.glb glb_structure
import bpy
bpy.data.materials.new('Red')
bpy.data.materials['Red'].diffuse_color = (1.0, 0.0, 0.0, 1.0)
bpy.data.materials.new('Blue')
bpy.data.materials['Blue'].diffuse_color = (0.0, 0.0, 1.0, 1.0)
bpy.context.preferences.addons['io_scene_gltf2'].preferences.KHR_materials_variants_ui = True
bpy.data.scenes[0].gltf2_KHR_materials_variants_variants.add()
bpy.data.scenes[0].gltf2_KHR_materials_variants_variants[0].name = 'Warm'
bpy.data.scenes[0].gltf2_KHR_materials_variants_variants.add()
bpy.data.scenes[0].gltf2_KHR_materials_variants_variants[1].variant_idx = 1
bpy.data.scenes[0].gltf2_KHR_materials_variants_variants[1].name = 'Cold'
bpy.data.meshes['Cube'].gltf2_variant_mesh_data.add()
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[0].material = bpy.data.materials['Red']
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[0].variants.add()
bpy.data.meshes['Cube'].gltf2_variant_mesh_data.add()
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[1].material = bpy.data.materials['Blue']
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[1].variants.add()
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[1].variants[0].variant.variant_idx = 1
bpy.data.meshes['Cube'].gltf2_variant_default_materials.add()
bpy.data.meshes['Cube'].gltf2_variant_default_materials[0].default_material = bpy.data.materials['Red']
bpy.data.objects['Cube'].material_slots[0].material = bpy.data.materials['Blue']
bpy.ops.export_scene.gltf(filepath='out/a.glb')
bpy.data.objects['Camera'].name = bpy.data.objects['Cube'].material_slots[0].material.name
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.context.preferences.addons['io_scene_gltf2'].preferences.KHR_materials_variants_ui = False
bpy.ops.import_scene.gltf(filepath='out/a.glb')
bpy.data.objects['Cube'].location = (bpy.context.preferences.addons['io_scene_gltf2'].preferences.KHR_materials_variants_ui, bpy.data.meshes['Cube'].gltf2_variant_mesh_data[1].variants[0].variant.variant_idx, bpy.data.meshes['Cube'].gltf2_variant_default_materials[0].material_slot_index)
bpy.data.objects['Cube'].name = bpy.data.scenes[0].gltf2_KHR_materials_variants_variants[1].name + '|' + bpy.data.meshes['Cube'].gltf2_variant_mesh_data[0].material.name + '|' + bpy.data.meshes['Cube'].gltf2_variant_default_materials[0].default_material.name
bpy.ops.export_scene.gltf(filepath='out/b.glb')
