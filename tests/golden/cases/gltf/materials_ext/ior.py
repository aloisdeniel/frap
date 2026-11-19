# WP 3.6: KHR_materials_ior export, written only with KHR_materials_transmission,
# KHR_materials_volume or KHR_materials_specular [GLTF §4.5] and not at 1.5: the first material
# (IOR 1.45 alone) has no extension, the second (IOR 1.33 + specular) and the third (IOR 2.0 +
# transmission) have it, the fourth (transmission, IOR 1.5) only has transmission.
# golden: startup=factory tier=T2 compare=glb:out/ior.glb
import bpy
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['IOR'].default_value = 1.45
bpy.data.materials.new('Water')
bpy.data.materials['Water'].node_tree.nodes['Principled BSDF'].inputs['IOR'].default_value = 1.33
bpy.data.materials['Water'].node_tree.nodes['Principled BSDF'].inputs['Specular IOR Level'].default_value = 0.25
bpy.data.materials.new('Diamond')
bpy.data.materials['Diamond'].node_tree.nodes['Principled BSDF'].inputs['IOR'].default_value = 2.0
bpy.data.materials['Diamond'].node_tree.nodes['Principled BSDF'].inputs['Transmission Weight'].default_value = 1.0
bpy.data.materials.new('Glass')
bpy.data.materials['Glass'].node_tree.nodes['Principled BSDF'].inputs['Transmission Weight'].default_value = 1.0
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.context.object.data.materials.append(bpy.data.materials['Water'])
bpy.ops.mesh.primitive_plane_add(location=(-3, 0, 0))
bpy.context.object.data.materials.append(bpy.data.materials['Diamond'])
bpy.ops.mesh.primitive_plane_add(location=(0, 3, 0))
bpy.context.object.data.materials.append(bpy.data.materials['Glass'])
bpy.ops.export_scene.gltf(filepath='out/ior.glb')
