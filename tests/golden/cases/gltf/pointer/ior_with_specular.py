# WP 6.9: __check_ior after __fix_json with pointer animation, through the specular extension:
# the static IOR is 1.3 and only the Specular Tint is animated, so the kept empty
# KHR_materials_specular object brings back export_ior's {"ior": 1.3} and its declaration, in
# the add-on's extensionsUsed order (specular, ior, animation pointer).
# golden: startup=factory tier=T2 compare=glb:out/spec_ior.glb glb_structure
import bpy
bpy.context.scene.frame_end = 8
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['IOR'].default_value = 1.3
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Specular Tint'].default_value = (1.0, 1.0, 1.0, 1.0)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Specular Tint'].keyframe_insert('default_value', frame=1)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Specular Tint'].default_value = (0.5, 0.5, 0.5, 1.0)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Specular Tint'].keyframe_insert('default_value', frame=8)
bpy.ops.export_scene.gltf(filepath='out/spec_ior.glb', export_animation_mode='SCENE', export_pointer_animation=True)
