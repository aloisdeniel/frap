# WP 7.5: the glTF add-on's KHR_materials_variants RNA (§4.1). With the factory preferences
# the properties do not exist (`AttributeError`); turning `KHR_materials_variants_ui` on
# registers `Scene.gltf2_KHR_materials_variants_variants` (an ID-property collection:
# `add()`, `remove(i)`, `move(a, b)`, `clear()`, items by index, looked up by name),
# `Scene.gltf2_active_variant` and the per-mesh groups. A variant's name is made unique by the
# add-on's update callback, among `bpy.data.scenes[0]`'s variants except the one at the
# position given by its `variant_idx`. Material pointers count users. Values are copied into
# object names and transforms (compare=objects).
# golden: startup=factory tier=T0 compare=objects
import bpy
bpy.data.scenes[0].gltf2_KHR_materials_variants_variants.add()
bpy.data.meshes['Cube'].gltf2_variant_mesh_data
bpy.data.scenes[0].gltf2_active_variant = 1
bpy.context.preferences.addons['io_scene_gltf2'].preferences.KHR_materials_variants_ui = True
bpy.data.scenes[0].gltf2_KHR_materials_variants_variants.add()
bpy.data.scenes[0].gltf2_KHR_materials_variants_variants[0].name = 'Red'
bpy.data.scenes[0].gltf2_KHR_materials_variants_variants.add()
bpy.data.scenes[0].gltf2_KHR_materials_variants_variants[1].variant_idx = 1
bpy.data.scenes[0].gltf2_KHR_materials_variants_variants[1].name = 'Red'
bpy.data.scenes[0].gltf2_KHR_materials_variants_variants.add()
bpy.data.scenes[0].gltf2_KHR_materials_variants_variants[2].variant_idx = 2
bpy.data.scenes[0].gltf2_KHR_materials_variants_variants[2].name = 'x' * 70
bpy.data.scenes[0].gltf2_KHR_materials_variants_variants.add(1)
bpy.data.scenes[0].gltf2_KHR_materials_variants_variants.remove()
bpy.data.scenes[0].gltf2_KHR_materials_variants_variants.remove('a')
bpy.data.scenes[0].gltf2_KHR_materials_variants_variants.remove(7)
bpy.data.scenes[0].gltf2_KHR_materials_variants_variants.move(0)
bpy.data.scenes[0].gltf2_KHR_materials_variants_variants.move(0, 9)
bpy.data.scenes[0].gltf2_KHR_materials_variants_variants.clear(1)
bpy.data.scenes[0].gltf2_KHR_materials_variants_variants.foo()
bpy.data.scenes[0].gltf2_KHR_materials_variants_variants = 3
bpy.data.scenes[0].gltf2_active_variant = 'x'
bpy.data.scenes[0].gltf2_active_variant = 2
bpy.data.objects['Cube'].name = bpy.data.scenes[0].gltf2_KHR_materials_variants_variants[1].name + '|' + bpy.data.scenes[0].gltf2_KHR_materials_variants_variants[2].name + '|' + bpy.data.scenes[0].gltf2_KHR_materials_variants_variants['Red'].path_from_id()
bpy.data.scenes[0].gltf2_KHR_materials_variants_variants.move(2, 0)
bpy.data.objects['Light'].name = bpy.data.scenes[0].gltf2_KHR_materials_variants_variants[0].name[0:3] + '|' + bpy.data.scenes[0].gltf2_KHR_materials_variants_variants[2].name
bpy.data.objects['Camera'].rotation_euler = (bpy.data.scenes[0].gltf2_KHR_materials_variants_variants.find('Red.001'), bpy.data.scenes[0].gltf2_KHR_materials_variants_variants[0].variant_idx, 0.0)
bpy.data.meshes['Cube'].gltf2_variant_mesh_data.add()
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[0].material = bpy.data.materials['Material']
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[0].material = bpy.data.meshes['Cube']
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[0].material_slot_index = 2 ** 40
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[0].material_slot_index = 3
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[0].variants.add()
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[0].variants[0].variant.variant_idx = 1
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[0].variants[0].variant.name = 'Blue'
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[0].variants[0].variant = 3
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[0].variants = 3
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[0].name = 'Prim'
bpy.data.meshes['Cube'].gltf2_variant_default_materials.add()
bpy.data.meshes['Cube'].gltf2_variant_default_materials[0].default_material = bpy.data.materials['Material']
bpy.data.meshes['Cube'].gltf2_variant_pointer = 'Blue'
bpy.data.objects['Camera'].location = (bpy.data.materials['Material'].users, bpy.data.meshes['Cube'].gltf2_variant_mesh_data[0].material_slot_index, bpy.data.scenes[0].gltf2_active_variant)
bpy.data.objects['Camera'].name = bpy.data.meshes['Cube'].gltf2_variant_mesh_data['Prim'].variants[0].variant.name + '|' + bpy.data.meshes['Cube'].gltf2_variant_mesh_data[0].material.name + '|' + bpy.data.meshes['Cube'].gltf2_variant_pointer + '|' + bpy.data.meshes['Cube'].gltf2_variant_default_materials[0].default_material.name + '|' + bpy.data.meshes['Cube'].gltf2_variant_mesh_data[0].variants[0].variant.path_from_id()
bpy.data.meshes['Cube'].gltf2_variant_mesh_data.remove(0)
bpy.data.scenes[0].gltf2_KHR_materials_variants_variants.clear()
bpy.context.scene.camera.scale = (bpy.data.materials['Material'].users, bpy.data.scenes[0].gltf2_KHR_materials_variants_variants.find('Red'), 1.0)
