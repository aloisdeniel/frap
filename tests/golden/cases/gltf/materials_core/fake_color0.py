# WP 3.5 (material side) + WP 3.7 (COLOR_0 writer). A colour attribute that no material uses: with
# `export_all_vertex_colors=True` (the default) the add-on inserts a white COLOR_0 (unsigned
# byte) so that the attribute, written as COLOR_1, does not tint the material; a material whose
# Base Color is a Color Attribute node (lowered as `LoweredMaterial::vertex_color`, WP 3.5)
# gets the attribute itself as COLOR_0 (float VEC3: only Base Color reads it); when the Color
# Attribute node's Alpha also feeds the Principled Alpha (so the material is BLEND), COLOR_0
# carries the alpha (normalized unsigned short VEC4; `VertexColorUse::adds_alpha`).
# golden: startup=factory tier=T2 compare=glb:out/fake.glb,glb:out/used.glb,glb:out/alpha.glb glb_structure
import bpy
bpy.data.meshes['Cube'].color_attributes.new('Col', 'BYTE_COLOR', 'CORNER')
bpy.ops.export_scene.gltf(filepath='out/fake.glb')
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeVertexColor')
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Color Attribute'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.ops.export_scene.gltf(filepath='out/used.glb')
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Color Attribute'].outputs['Alpha'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Alpha'])
bpy.ops.export_scene.gltf(filepath='out/alpha.glb')
