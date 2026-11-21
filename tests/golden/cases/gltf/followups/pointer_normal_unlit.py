# WP 7.7: KHR_animation_pointer channels the add-on registers beyond the factors: the Normal
# Map's Strength as /materials/i/normalTexture/scale (texture_info.py __gather_normal_scale,
# only when the normal texture is written), and an unlit material's colour (unlit.py
# gather_base_color_factor: a Background shader's Color as baseColorFactor).
# golden: startup=factory tier=T2 compare=glb:out/normal_unlit.glb glb_structure
import bpy
bpy.context.scene.frame_end = 6
bpy.data.images.new('N', 4, 4)
bpy.data.images['N'].generated_color = (0.5, 0.5, 1.0, 1.0)
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['N']
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeNormalMap')
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Normal Map'].inputs['Color'])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Normal Map'].outputs['Normal'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Normal'])
bpy.data.materials['Material'].node_tree.nodes['Normal Map'].inputs['Strength'].default_value = 0.5
bpy.data.materials['Material'].node_tree.nodes['Normal Map'].inputs['Strength'].keyframe_insert('default_value', frame=1)
bpy.data.materials['Material'].node_tree.nodes['Normal Map'].inputs['Strength'].default_value = 2.0
bpy.data.materials['Material'].node_tree.nodes['Normal Map'].inputs['Strength'].keyframe_insert('default_value', frame=6)
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.context.object.data.materials.append(bpy.data.materials.new('Flat'))
bpy.data.materials['Flat'].node_tree.nodes.new('ShaderNodeBackground')
bpy.data.materials['Flat'].node_tree.links.new(bpy.data.materials['Flat'].node_tree.nodes['Background'].outputs[0], bpy.data.materials['Flat'].node_tree.nodes['Material Output'].inputs['Surface'])
bpy.data.materials['Flat'].node_tree.nodes['Background'].inputs['Color'].default_value = (1.0, 0.0, 0.0, 1.0)
bpy.data.materials['Flat'].node_tree.nodes['Background'].inputs['Color'].keyframe_insert('default_value', frame=1)
bpy.data.materials['Flat'].node_tree.nodes['Background'].inputs['Color'].default_value = (0.0, 0.5, 1.0, 1.0)
bpy.data.materials['Flat'].node_tree.nodes['Background'].inputs['Color'].keyframe_insert('default_value', frame=6)
bpy.ops.export_scene.gltf(filepath='out/normal_unlit.glb', export_animation_mode='SCENE', export_pointer_animation=True)
