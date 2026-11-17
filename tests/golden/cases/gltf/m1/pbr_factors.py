# WP 1.23: Principled BSDF base colour, metallic and roughness factors on two materials.
# golden: startup=factory tier=T2 compare=glb:out/pbr.glb
import bpy
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.2, 0.5, 0.8, 1)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 0.75
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.2
bpy.data.materials.new('Steel')
bpy.data.materials['Steel'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 1.0
bpy.data.materials['Steel'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.35
bpy.data.materials['Steel'].diffuse_color = (0.6, 0.6, 0.7, 1.0)
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.context.object.data.materials.append(bpy.data.materials['Steel'])
bpy.ops.export_scene.gltf(filepath='out/pbr.glb')
