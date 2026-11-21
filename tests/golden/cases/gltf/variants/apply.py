# WP 7.5: KHR_materials_variants with `export_apply=True`. The add-on gathers an object with
# modifiers from `evaluated_get(depsgraph).to_mesh()`, a temporary mesh that keeps the data's
# `gltf2_variant_mesh_data` only when the stack passes the evaluated copy through: without
# modifiers the mappings are written (`a.glb`); a Bevel builds a new mesh, so the file has no
# variants at all, not even the root list or `extensionsUsed` (`b.glb`); the same Bevel off in
# the viewport (`c.glb`), a Displace (a deform modifier, `d.glb`) and a Subdivision with
# `levels = 0` (`e.glb`) keep them; a Triangulate after the Displace drops them (`f.glb`).
# golden: startup=factory tier=T2 compare=glb:out/a.glb,glb:out/b.glb,glb:out/c.glb,glb:out/d.glb,glb:out/e.glb,glb:out/f.glb glb_structure
import bpy
bpy.data.materials.new('Red')
bpy.context.preferences.addons['io_scene_gltf2'].preferences.KHR_materials_variants_ui = True
bpy.data.scenes[0].gltf2_KHR_materials_variants_variants.add()
bpy.data.scenes[0].gltf2_KHR_materials_variants_variants[0].name = 'V0'
bpy.data.meshes['Cube'].gltf2_variant_mesh_data.add()
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[0].material = bpy.data.materials['Red']
bpy.data.meshes['Cube'].gltf2_variant_mesh_data[0].variants.add()
bpy.ops.export_scene.gltf(filepath='out/a.glb', export_apply=True)
bpy.ops.object.modifier_add(type='BEVEL')
bpy.ops.export_scene.gltf(filepath='out/b.glb', export_apply=True)
bpy.data.objects['Cube'].modifiers['Bevel'].show_viewport = False
bpy.ops.export_scene.gltf(filepath='out/c.glb', export_apply=True)
bpy.ops.object.modifier_remove(modifier='Bevel')
bpy.ops.object.modifier_add(type='DISPLACE')
bpy.ops.export_scene.gltf(filepath='out/d.glb', export_apply=True)
bpy.ops.object.modifier_remove(modifier='Displace')
bpy.ops.object.modifier_add(type='SUBSURF')
bpy.data.objects['Cube'].modifiers['Subdivision'].levels = 0
bpy.ops.export_scene.gltf(filepath='out/e.glb', export_apply=True)
bpy.ops.object.modifier_remove(modifier='Subdivision')
bpy.ops.object.modifier_add(type='DISPLACE')
bpy.ops.object.modifier_add(type='TRIANGULATE')
bpy.ops.export_scene.gltf(filepath='out/f.glb', export_apply=True)
