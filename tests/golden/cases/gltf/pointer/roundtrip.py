# WP 6.9: KHR_animation_pointer round trip. Material (embedded node tree), spot light and
# camera property animation is exported in SCENE mode, the file is imported into an empty
# file: one action per glTF animation (named after it) with one slot per datablock (NTSlot,
# LASlot, CASlot), F-curves at time x fps (base colour + alpha, roughness, emission strength in
# the Principled sockets; energy back through the directional conversion, so the export's
# x 683 cancels; spot_size from the outer cone; spot_blend = 1 - inner / outer from the raw
# blend the export wrote; lens from yfov with the imported sensor height; ortho_scale from
# xmag and ymag together; clip distances), stashed on muted, locked NLA tracks and assigned
# back. The values at frame 6 and the stashed tracks are copied into transforms and names,
# then the import is exported again (SCENE: only the restored first animation is active; NLA
# tracks: every stashed track; last, as Blender's NLA bake leaves the baked values in the
# unanimated datablocks, which frap's export clone does not, D-07).
# golden: startup=factory tier=T2 compare=anim:Material,anim:Light,anim:Camera,anim:Ortho,objects,glb:out/b.glb,glb:out/c.glb
import bpy
bpy.context.scene.frame_end = 10
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (1.0, 0.0, 0.0, 1.0)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].keyframe_insert('default_value', frame=1)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.0, 0.25, 1.0, 1.0)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].keyframe_insert('default_value', frame=10)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Alpha'].default_value = 0.9
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Alpha'].keyframe_insert('default_value', frame=1)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Alpha'].default_value = 0.3
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Alpha'].keyframe_insert('default_value', frame=10)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.1
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].keyframe_insert('default_value', frame=1)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.9
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].keyframe_insert('default_value', frame=10)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Emission Color'].default_value = (1.0, 1.0, 1.0, 1.0)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Emission Strength'].default_value = 0.2
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Emission Strength'].keyframe_insert('default_value', frame=1)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Emission Strength'].default_value = 0.8
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Emission Strength'].keyframe_insert('default_value', frame=10)
bpy.data.lights['Light'].type = 'SPOT'
bpy.data.lights['Light'].energy = 200.0
bpy.data.lights['Light'].keyframe_insert('energy', frame=1)
bpy.data.lights['Light'].energy = 800.0
bpy.data.lights['Light'].keyframe_insert('energy', frame=10)
bpy.data.lights['Light'].color = (1.0, 1.0, 1.0)
bpy.data.lights['Light'].keyframe_insert('color', frame=1)
bpy.data.lights['Light'].color = (0.2, 0.6, 1.0)
bpy.data.lights['Light'].keyframe_insert('color', frame=10)
bpy.data.lights['Light'].spot_size = 0.6
bpy.data.lights['Light'].spot_blend = 0.1
bpy.data.lights['Light'].keyframe_insert('spot_size', frame=1)
bpy.data.lights['Light'].keyframe_insert('spot_blend', frame=1)
bpy.data.lights['Light'].spot_size = 1.2
bpy.data.lights['Light'].spot_blend = 0.3
bpy.data.lights['Light'].keyframe_insert('spot_size', frame=10)
bpy.data.lights['Light'].keyframe_insert('spot_blend', frame=10)
bpy.data.cameras['Camera'].lens = 35.0
bpy.data.cameras['Camera'].keyframe_insert('lens', frame=1)
bpy.data.cameras['Camera'].lens = 85.0
bpy.data.cameras['Camera'].keyframe_insert('lens', frame=10)
bpy.data.cameras['Camera'].clip_end = 50.0
bpy.data.cameras['Camera'].keyframe_insert('clip_end', frame=1)
bpy.data.cameras['Camera'].clip_end = 300.0
bpy.data.cameras['Camera'].keyframe_insert('clip_end', frame=10)
bpy.ops.object.camera_add(location=(4, -4, 2))
bpy.context.object.name = 'Side'
bpy.context.object.data.name = 'Ortho'
bpy.data.cameras['Ortho'].type = 'ORTHO'
bpy.data.cameras['Ortho'].ortho_scale = 4.0
bpy.data.cameras['Ortho'].keyframe_insert('ortho_scale', frame=1)
bpy.data.cameras['Ortho'].ortho_scale = 10.0
bpy.data.cameras['Ortho'].keyframe_insert('ortho_scale', frame=10)
bpy.ops.export_scene.gltf(filepath='out/a.glb', export_animation_mode='SCENE', export_pointer_animation=True, export_lights=True, export_cameras=True)
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/a.glb')
bpy.context.scene.frame_set(6)
bpy.data.objects['Light'].location = (bpy.data.lights['Light'].energy, bpy.data.lights['Light'].spot_size, bpy.data.lights['Light'].spot_blend)
bpy.data.objects['Light'].scale = bpy.data.lights['Light'].color
bpy.data.objects['Camera'].location = (bpy.data.cameras['Camera'].lens, bpy.data.cameras['Camera'].clip_end, bpy.data.cameras['Camera'].clip_start)
bpy.data.objects['Side'].location = (bpy.data.cameras['Ortho'].ortho_scale, bpy.data.cameras['Ortho'].clip_end, 0.0)
bpy.data.objects['Cube'].location = bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value[0:3]
bpy.data.objects['Cube'].scale = (bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Alpha'].default_value, bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value, bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Emission Strength'].default_value)
bpy.ops.object.empty_add(location=(bpy.data.lights['Light'].animation_data.nla_tracks[0].strips[0].frame_start, bpy.data.lights['Light'].animation_data.nla_tracks[0].strips[0].frame_end, bpy.data.lights['Light'].animation_data.nla_tracks[0].mute + 2 * bpy.data.lights['Light'].animation_data.nla_tracks[0].lock))
bpy.context.object.name = bpy.data.lights['Light'].animation_data.nla_tracks[0].name + '|' + bpy.data.lights['Light'].animation_data.nla_tracks[0].strips[0].name + '|' + bpy.data.lights['Light'].animation_data.nla_tracks[0].strips[0].action_slot.identifier
bpy.ops.object.empty_add(location=(bpy.data.materials['Material'].node_tree.animation_data.nla_tracks[0].strips[0].action_frame_start, bpy.data.materials['Material'].node_tree.animation_data.nla_tracks[0].strips[0].action_frame_end, bpy.data.actions['Material'].slots[0].handle))
bpy.context.object.name = bpy.data.materials['Material'].node_tree.animation_data.action.name + '|' + bpy.data.materials['Material'].node_tree.animation_data.action_slot.identifier + '|' + bpy.data.actions['Material'].layers[0].name
bpy.context.scene.frame_set(1)
bpy.ops.export_scene.gltf(filepath='out/b.glb', export_animation_mode='SCENE', export_pointer_animation=True, export_lights=True, export_cameras=True)
bpy.ops.export_scene.gltf(filepath='out/c.glb', export_animation_mode='NLA_TRACKS', export_pointer_animation=True, export_lights=True, export_cameras=True)
