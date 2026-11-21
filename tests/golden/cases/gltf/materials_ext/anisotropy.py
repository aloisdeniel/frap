# WP 3.6: KHR_materials_anisotropy export from unlinked sockets: anisotropyStrength =
# Anisotropic, anisotropyRotation = Anisotropic Rotation x 2 pi; a second material with only a
# rotation (strength 0, not written) and a third with neither (no extension).
# golden: startup=factory tier=T2 compare=glb:out/anisotropy.glb glb_structure
import bpy
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Anisotropic'].default_value = 0.5
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Anisotropic Rotation'].default_value = 0.125
bpy.data.materials.new('Brushed')
bpy.data.materials['Brushed'].node_tree.nodes['Principled BSDF'].inputs['Anisotropic Rotation'].default_value = 0.3
bpy.data.materials.new('Smooth')
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.context.object.data.materials.append(bpy.data.materials['Brushed'])
bpy.ops.mesh.primitive_plane_add(location=(-3, 0, 0))
bpy.context.object.data.materials.append(bpy.data.materials['Smooth'])
bpy.ops.export_scene.gltf(filepath='out/anisotropy.glb')
