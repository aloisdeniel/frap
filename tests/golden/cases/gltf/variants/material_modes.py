# WP 7.5: KHR_materials_variants mappings and `export_materials`. A mapping's material goes
# through `gather_material` itself, which ignores 'PLACEHOLDER': the primitive has no material
# but the mapping keeps Red (`a.glb`, `materials: [Red]`). With 'NONE' the single primitive has
# no material slot, so no mapping matches and nothing is written (`b.glb`); 'VIEWPORT' gathers
# both as viewport materials (`c.glb`).
# golden: startup=factory tier=T2 compare=glb:out/a.glb,glb:out/b.glb,glb:out/c.glb glb_structure
import bpy
bpy.data.materials.new('Red')
bpy.data.materials['Red'].diffuse_color = (1.0, 0.0, 0.0, 1.0)
bpy.context.preferences.addons['io_scene_gltf2'].preferences.KHR_materials_variants_ui = True
bpy.data.scenes[0].gltf2_KHR_materials_variants_variants.add()
bpy.data.scenes[0].gltf2_KHR_materials_variants_variants[0].name = 'V0'
bpy.data.meshes['Cube'].gltf2_variant_mesh_data.add()
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[0].material = bpy.data.materials['Red']
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[0].variants.add()
bpy.ops.export_scene.gltf(filepath='out/a.glb', export_materials='PLACEHOLDER')
bpy.ops.export_scene.gltf(filepath='out/b.glb', export_materials='NONE')
bpy.ops.export_scene.gltf(filepath='out/c.glb', export_materials='VIEWPORT')
