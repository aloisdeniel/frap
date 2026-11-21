# WP 7.5: KHR_materials_variants export (`__gather_extensions`). Two material slots on the
# cube; the scene's variants Red, Blue, Gold; per-slot mappings in
# `Mesh.gltf2_variant_mesh_data` (slot 0: Red for variants 0 and 2, Blue for variant 1, an
# empty-slot entry for variant 2 again, which the add-on skips with its "2 different
# materials" warning; slot 1: Gold for variant 2 listed twice, kept once). The root list holds
# the variants in first-use order. Without the preference nothing is written (`off.glb`); a
# cube copy sharing the mesh shares the primitives and their mappings.
# golden: startup=factory tier=T2 compare=glb:out/variants.glb,glb:out/off.glb glb_structure
import bpy
bpy.data.materials.new('Red')
bpy.data.materials['Red'].diffuse_color = (1.0, 0.0, 0.0, 1.0)
bpy.data.materials.new('Blue')
bpy.data.materials['Blue'].diffuse_color = (0.0, 0.0, 1.0, 1.0)
bpy.data.materials.new('Gold')
bpy.data.materials['Gold'].metallic = 1.0
bpy.data.meshes['Cube'].materials.append(bpy.data.materials['Blue'])
bpy.data.meshes['Cube'].polygons[0].material_index = 1
bpy.data.meshes['Cube'].polygons[1].material_index = 1
bpy.data.objects.new('Copy', bpy.data.meshes['Cube'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Copy'])
bpy.data.objects['Copy'].location = (3, 0, 0)
bpy.ops.export_scene.gltf(filepath='out/off.glb')
bpy.context.preferences.addons['io_scene_gltf2'].preferences.KHR_materials_variants_ui = True
bpy.data.scenes[0].gltf2_KHR_materials_variants_variants.add()
bpy.data.scenes[0].gltf2_KHR_materials_variants_variants[0].name = 'Red'
bpy.data.scenes[0].gltf2_KHR_materials_variants_variants.add()
bpy.data.scenes[0].gltf2_KHR_materials_variants_variants[1].variant_idx = 1
bpy.data.scenes[0].gltf2_KHR_materials_variants_variants[1].name = 'Blue'
bpy.data.scenes[0].gltf2_KHR_materials_variants_variants.add()
bpy.data.scenes[0].gltf2_KHR_materials_variants_variants[2].variant_idx = 2
bpy.data.scenes[0].gltf2_KHR_materials_variants_variants[2].name = 'Gold'
bpy.data.meshes['Cube'].gltf2_variant_mesh_data.add()
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[0].material = bpy.data.materials['Blue']
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[0].variants.add()
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[0].variants[0].variant.variant_idx = 1
bpy.data.meshes['Cube'].gltf2_variant_mesh_data.add()
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[1].material = bpy.data.materials['Red']
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[1].variants.add()
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[1].variants.add()
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[1].variants[1].variant.variant_idx = 2
bpy.data.meshes['Cube'].gltf2_variant_mesh_data.add()
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[2].variants.add()
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[2].variants[0].variant.variant_idx = 2
bpy.data.meshes['Cube'].gltf2_variant_mesh_data.add()
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[3].material_slot_index = 1
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[3].material = bpy.data.materials['Gold']
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[3].variants.add()
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[3].variants[0].variant.variant_idx = 2
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[3].variants.add()
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[3].variants[1].variant.variant_idx = 2
bpy.ops.export_scene.gltf(filepath='out/variants.glb')
