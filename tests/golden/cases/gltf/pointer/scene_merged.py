# WP 6.9: KHR_animation_pointer in SCENE mode with export_anim_scene_split_object=False: every
# pointer channel in the scene's animation (scene extras with export_extras); frame_start 3 to
# frame_end 11 every 2 frames (export_frame_step), slid to 0 (export_anim_slide_to_zero: times
# (frame - 3) / fps), the fps_base quirk otherwise (30 fps, fps_base 1.001); STEP sampling;
# the COMPAT lighting mode (no x 683) on a sun; a second, static material that keeps the empty
# KHR_materials_emissive_strength / KHR_materials_clearcoat objects because another material
# animates those extensions (the add-on's check is global); a material whose node tree has
# animation data but nothing animated exports no channel.
# golden: startup=factory tier=T2 compare=glb:out/scene_merged.glb,glb:out/scene_fps.glb
import bpy
bpy.context.scene.frame_start = 3
bpy.context.scene.frame_end = 11
bpy.context.scene.render.fps = 30
bpy.context.scene.render.fps_base = 1.001
bpy.data.scenes['Scene']['note'] = 'pointer'
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Emission Strength'].default_value = 0.5
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Emission Strength'].keyframe_insert('default_value', frame=3)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Emission Strength'].default_value = 3.0
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Emission Strength'].keyframe_insert('default_value', frame=11)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Coat Weight'].default_value = 0.0
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Coat Weight'].keyframe_insert('default_value', frame=5)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Coat Weight'].default_value = 1.0
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Coat Weight'].keyframe_insert('default_value', frame=9)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 0.0
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].keyframe_insert('default_value', frame=4)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 1.0
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].keyframe_insert('default_value', frame=10)
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.data.materials.new('Static')
bpy.context.object.data.materials.append(bpy.data.materials['Static'])
bpy.ops.mesh.primitive_plane_add(location=(-3, 0, 0))
bpy.data.materials.new('Keyed')
bpy.context.object.data.materials.append(bpy.data.materials['Keyed'])
bpy.data.materials['Keyed'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].keyframe_insert('default_value', frame=3)
bpy.data.lights['Light'].type = 'SUN'
bpy.data.lights['Light'].energy = 2.0
bpy.data.lights['Light'].keyframe_insert('energy', frame=3)
bpy.data.lights['Light'].energy = 6.0
bpy.data.lights['Light'].keyframe_insert('energy', frame=9)
bpy.data.cameras['Camera'].clip_start = 0.5
bpy.data.cameras['Camera'].keyframe_insert('clip_start', frame=5)
bpy.data.cameras['Camera'].clip_start = 0.05
bpy.data.cameras['Camera'].keyframe_insert('clip_start', frame=7)
bpy.ops.export_scene.gltf(filepath='out/scene_merged.glb', export_animation_mode='SCENE', export_anim_scene_split_object=False, export_pointer_animation=True, export_lights=True, export_cameras=True, export_frame_step=2, export_anim_slide_to_zero=True, export_sampling_interpolation_fallback='STEP', export_import_convert_lighting_mode='COMPAT', export_extras=True)
bpy.ops.export_scene.gltf(filepath='out/scene_fps.glb', export_animation_mode='SCENE', export_anim_scene_split_object=False, export_pointer_animation=True, export_lights=True)
