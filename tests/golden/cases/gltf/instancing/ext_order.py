# WP 7.5: `manage_gpu_instancing_nodes` runs after every scene and animation is traversed, so
# `EXT_mesh_gpu_instancing` is declared after the extensions of the instanced mesh's
# material (here `KHR_materials_emissive_strength`, an emission strength above 1).
# golden: startup=factory tier=T2 compare=glb:out/order.glb glb_structure
import bpy
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Emission Color'].default_value = (1.0, 0.5, 0.0, 1.0)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Emission Strength'].default_value = 4.0
bpy.ops.object.empty_add(location=(0, 0, 0))
bpy.data.objects['Cube'].parent = bpy.data.objects['Empty']
bpy.data.objects.new('Cube2', bpy.data.meshes['Cube'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Cube2'])
bpy.data.objects['Cube2'].parent = bpy.data.objects['Empty']
bpy.data.objects['Cube2'].location = (3, 0, 0)
bpy.ops.export_scene.gltf(filepath='out/order.glb', export_gpu_instances=True)
