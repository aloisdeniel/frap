# WP 7.7: with export_pointer_animation (SCENE mode) a material whose node tree is animated is
# read from its original tree (bmat.used == "ORIGINAL"): no constant folding, so a Math node
# computing 0.3 x 2 into Roughness leaves roughnessFactor at 1, and a Mix of constants into
# Base Color leaves the base colour white. A copy without animation keeps the same nodes and is
# inlined (0.6, the mixed colour). Without the pointer option both are inlined.
# golden: startup=factory tier=T2 compare=glb:out/pointer.glb,glb:out/plain.glb glb_structure
import bpy
bpy.context.scene.frame_end = 4
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeMath')
bpy.data.materials['Material'].node_tree.nodes['Math'].operation = 'MULTIPLY'
bpy.data.materials['Material'].node_tree.nodes['Math'].inputs[0].default_value = 0.3
bpy.data.materials['Material'].node_tree.nodes['Math'].inputs[1].default_value = 2.0
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Math'].outputs[0], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Roughness'])
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeMix')
bpy.data.materials['Material'].node_tree.nodes['Mix'].data_type = 'RGBA'
bpy.data.materials['Material'].node_tree.nodes['Mix'].inputs['Factor'].default_value = 0.5
bpy.data.materials['Material'].node_tree.nodes['Mix'].inputs['A'].default_value = (1.0, 0.0, 0.0, 1.0)
bpy.data.materials['Material'].node_tree.nodes['Mix'].inputs['B'].default_value = (0.0, 0.0, 1.0, 1.0)
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Mix'].outputs['Result'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.materials['Material'].copy()
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 0.0
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].keyframe_insert('default_value', frame=1)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 1.0
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].keyframe_insert('default_value', frame=4)
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.context.object.data.materials.append(bpy.data.materials['Material.001'])
bpy.ops.export_scene.gltf(filepath='out/pointer.glb', export_animation_mode='SCENE', export_pointer_animation=True)
bpy.ops.export_scene.gltf(filepath='out/plain.glb', export_animation_mode='SCENE')
