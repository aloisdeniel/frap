# WP 6.9: __check_ior after __fix_json with pointer animation. The material's static IOR is
# 1.3 and only its Transmission Weight is animated: the empty KHR_materials_transmission object
# is kept because it is animated, so export_ior's {"ior": 1.3} (created beside the
# always-created transmission object) survives too and KHR_materials_ior is declared. The
# second file also animates the IOR (1.2 at the current frame, 2.0 later): the object carries
# the value at the current frame, not an empty object.
# golden: startup=factory tier=T2 compare=glb:out/trans_ior.glb,glb:out/trans_ior_anim.glb
import bpy
bpy.context.scene.frame_end = 8
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['IOR'].default_value = 1.3
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Transmission Weight'].default_value = 0.0
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Transmission Weight'].keyframe_insert('default_value', frame=1)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Transmission Weight'].default_value = 1.0
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Transmission Weight'].keyframe_insert('default_value', frame=8)
bpy.ops.export_scene.gltf(filepath='out/trans_ior.glb', export_animation_mode='SCENE', export_pointer_animation=True)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['IOR'].default_value = 1.2
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['IOR'].keyframe_insert('default_value', frame=1)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['IOR'].default_value = 2.0
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['IOR'].keyframe_insert('default_value', frame=8)
bpy.context.scene.frame_set(1)
bpy.ops.export_scene.gltf(filepath='out/trans_ior_anim.glb', export_animation_mode='SCENE', export_pointer_animation=True)
