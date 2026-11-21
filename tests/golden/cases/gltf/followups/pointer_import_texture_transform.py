# WP 7.7: KHR_animation_pointer import of KHR_texture_transform and unlit colours (imp/animation_pointer.py
# do_channel). Exported in SCENE mode: the cube's base colour texture behind a POINT Mapping node (Rotation Z
# 0.3, Location and Scale keyed with the same keys), and an unlit material (the importer's light-path trick
# with a Transparent BSDF mix: Emission Color and the alpha Mix Shader's factor keyed, BLEND). Imported into
# an empty file: the offset channel keys the new Mapping node's Location through
# texture_transform_gltf_to_blender with the static rotation and the scale channel's values (the original
# Location comes back), the scale channel its Scale; the unlit colour keys the Emission Color, its alpha the
# Mix Shader's factor. Values at frame 3 are copied into transforms.
# golden: startup=factory tier=T2 compare=anim:Flat,objects
import bpy
bpy.context.scene.frame_end = 5
bpy.data.images.new('C', 4, 4)
bpy.data.images['C'].generated_color = (0.9, 0.6, 0.3, 1.0)
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['C']
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeMapping')
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Mapping'].outputs['Vector'], bpy.data.materials['Material'].node_tree.nodes['Image Texture'].inputs['Vector'])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.materials['Material'].node_tree.nodes['Mapping'].inputs['Rotation'].default_value = (0.0, 0.0, 0.3)
bpy.data.materials['Material'].node_tree.nodes['Mapping'].inputs['Scale'].default_value = (2.0, 2.0, 1.0)
bpy.data.materials['Material'].node_tree.nodes['Mapping'].inputs['Location'].keyframe_insert('default_value', frame=1)
bpy.data.materials['Material'].node_tree.nodes['Mapping'].inputs['Scale'].keyframe_insert('default_value', frame=1)
bpy.data.materials['Material'].node_tree.nodes['Mapping'].inputs['Location'].default_value = (0.5, 0.25, 0.0)
bpy.data.materials['Material'].node_tree.nodes['Mapping'].inputs['Scale'].default_value = (3.0, 1.5, 1.0)
bpy.data.materials['Material'].node_tree.nodes['Mapping'].inputs['Location'].keyframe_insert('default_value', frame=5)
bpy.data.materials['Material'].node_tree.nodes['Mapping'].inputs['Scale'].keyframe_insert('default_value', frame=5)
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.context.object.data.materials.append(bpy.data.materials.new('Flat'))
bpy.data.materials['Flat'].node_tree.nodes.remove(bpy.data.materials['Flat'].node_tree.nodes['Principled BSDF'])
bpy.data.materials['Flat'].node_tree.nodes.new('ShaderNodeEmission')
bpy.data.materials['Flat'].node_tree.nodes.new('ShaderNodeLightPath')
bpy.data.materials['Flat'].node_tree.nodes.new('ShaderNodeBsdfTransparent')
bpy.data.materials['Flat'].node_tree.nodes.new('ShaderNodeMixShader')
bpy.data.materials['Flat'].node_tree.nodes.new('ShaderNodeMixShader')
bpy.data.materials['Flat'].node_tree.links.new(bpy.data.materials['Flat'].node_tree.nodes['Light Path'].outputs['Is Camera Ray'], bpy.data.materials['Flat'].node_tree.nodes['Mix Shader'].inputs[0])
bpy.data.materials['Flat'].node_tree.links.new(bpy.data.materials['Flat'].node_tree.nodes['Transparent BSDF'].outputs[0], bpy.data.materials['Flat'].node_tree.nodes['Mix Shader'].inputs[1])
bpy.data.materials['Flat'].node_tree.links.new(bpy.data.materials['Flat'].node_tree.nodes['Emission'].outputs[0], bpy.data.materials['Flat'].node_tree.nodes['Mix Shader'].inputs[2])
bpy.data.materials['Flat'].node_tree.links.new(bpy.data.materials['Flat'].node_tree.nodes['Transparent BSDF'].outputs[0], bpy.data.materials['Flat'].node_tree.nodes['Mix Shader.001'].inputs[1])
bpy.data.materials['Flat'].node_tree.links.new(bpy.data.materials['Flat'].node_tree.nodes['Mix Shader'].outputs[0], bpy.data.materials['Flat'].node_tree.nodes['Mix Shader.001'].inputs[2])
bpy.data.materials['Flat'].node_tree.links.new(bpy.data.materials['Flat'].node_tree.nodes['Mix Shader.001'].outputs[0], bpy.data.materials['Flat'].node_tree.nodes['Material Output'].inputs['Surface'])
bpy.data.materials['Flat'].node_tree.nodes['Emission'].inputs['Color'].default_value = (1.0, 0.0, 0.0, 1.0)
bpy.data.materials['Flat'].node_tree.nodes['Emission'].inputs['Color'].keyframe_insert('default_value', frame=1)
bpy.data.materials['Flat'].node_tree.nodes['Emission'].inputs['Color'].default_value = (0.0, 0.5, 1.0, 1.0)
bpy.data.materials['Flat'].node_tree.nodes['Emission'].inputs['Color'].keyframe_insert('default_value', frame=5)
bpy.data.materials['Flat'].node_tree.nodes['Mix Shader.001'].inputs[0].default_value = 0.8
bpy.data.materials['Flat'].node_tree.nodes['Mix Shader.001'].inputs[0].keyframe_insert('default_value', frame=1)
bpy.data.materials['Flat'].node_tree.nodes['Mix Shader.001'].inputs[0].default_value = 0.3
bpy.data.materials['Flat'].node_tree.nodes['Mix Shader.001'].inputs[0].keyframe_insert('default_value', frame=5)
bpy.context.scene.frame_set(1)
bpy.ops.export_scene.gltf(filepath='out/a.glb', export_animation_mode='SCENE', export_pointer_animation=True)
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/a.glb')
bpy.context.scene.frame_set(3)
bpy.data.objects['Cube'].location = bpy.data.materials['Material'].node_tree.nodes['Mapping'].inputs['Location'].default_value
bpy.data.objects['Cube'].scale = bpy.data.materials['Material'].node_tree.nodes['Mapping'].inputs['Scale'].default_value
bpy.data.objects['Plane'].location = bpy.data.materials['Flat'].node_tree.nodes['Emission'].inputs['Color'].default_value[0:3]
bpy.data.objects['Plane'].scale = (bpy.data.materials['Flat'].node_tree.nodes['Mix Shader.001'].inputs[0].default_value, 1.0, 1.0)
