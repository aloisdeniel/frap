# WP 6.9: KHR_animation_pointer in NLA_TRACKS mode. The material's node tree has a "Glow"
# track and an ADD track above it (grouped with it and baked together over both strips'
# range); the light has a "Glow" track too, so its channels merge into the material's
# animation, which takes the track's name; its active action is removed while baking; the
# camera's track keeps the default name (NlaTrack), so its animation is named after the camera.
# golden: startup=factory tier=T2 compare=glb:out/nla.glb
import bpy
bpy.data.actions.new('Pulse')
bpy.data.actions['Pulse'].slots.new(id_type='NODETREE', name='Shader Nodetree')
bpy.data.actions['Pulse'].layers.new('Layer').strips.new(type='KEYFRAME').channelbag(bpy.data.actions['Pulse'].slots[0], ensure=True).fcurves.new('nodes["Principled BSDF"].inputs[2].default_value').keyframe_points.insert(1, 0.1)
bpy.data.actions['Pulse'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points.insert(10, 0.7)
bpy.data.actions['Pulse'].layers[0].strips[0].channelbags[0].fcurves.new('nodes["Principled BSDF"].inputs[0].default_value', index=1).keyframe_points.insert(1, 0.2)
bpy.data.actions['Pulse'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points.insert(10, 0.9)
bpy.data.actions.new('Extra')
bpy.data.actions['Extra'].slots.new(id_type='NODETREE', name='Shader Nodetree')
bpy.data.actions['Extra'].layers.new('Layer').strips.new(type='KEYFRAME').channelbag(bpy.data.actions['Extra'].slots[0], ensure=True).fcurves.new('nodes["Principled BSDF"].inputs[2].default_value').keyframe_points.insert(4, 0.0)
bpy.data.actions['Extra'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points.insert(14, 0.2)
bpy.data.materials['Material'].node_tree.animation_data_create()
bpy.data.materials['Material'].node_tree.animation_data.nla_tracks.new()
bpy.data.materials['Material'].node_tree.animation_data.nla_tracks[0].name = 'Glow'
bpy.data.materials['Material'].node_tree.animation_data.nla_tracks[0].strips.new('Pulse', 1, bpy.data.actions['Pulse'])
bpy.data.materials['Material'].node_tree.animation_data.nla_tracks.new()
bpy.data.materials['Material'].node_tree.animation_data.nla_tracks[1].name = 'Boost'
bpy.data.materials['Material'].node_tree.animation_data.nla_tracks[1].strips.new('Extra', 4, bpy.data.actions['Extra'])
bpy.data.materials['Material'].node_tree.animation_data.nla_tracks[1].strips[0].blend_type = 'ADD'
bpy.data.actions.new('Flash')
bpy.data.actions['Flash'].slots.new(id_type='LIGHT', name='Light')
bpy.data.actions['Flash'].layers.new('Layer').strips.new(type='KEYFRAME').channelbag(bpy.data.actions['Flash'].slots[0], ensure=True).fcurves.new('energy').keyframe_points.insert(2, 100.0)
bpy.data.actions['Flash'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points.insert(8, 1500.0)
bpy.data.lights['Light'].animation_data_create()
bpy.data.lights['Light'].animation_data.nla_tracks.new()
bpy.data.lights['Light'].animation_data.nla_tracks[0].name = 'Glow'
bpy.data.lights['Light'].animation_data.nla_tracks[0].strips.new('Flash', 2, bpy.data.actions['Flash'])
bpy.data.lights['Light'].color = (1.0, 0.0, 0.0)
bpy.data.lights['Light'].keyframe_insert('color', frame=1)
bpy.data.lights['Light'].color = (0.0, 1.0, 0.0)
bpy.data.lights['Light'].keyframe_insert('color', frame=20)
bpy.data.actions.new('Zoom')
bpy.data.actions['Zoom'].slots.new(id_type='CAMERA', name='Camera')
bpy.data.actions['Zoom'].layers.new('Layer').strips.new(type='KEYFRAME').channelbag(bpy.data.actions['Zoom'].slots[0], ensure=True).fcurves.new('lens').keyframe_points.insert(1, 30.0)
bpy.data.actions['Zoom'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points.insert(6, 120.0)
bpy.data.cameras['Camera'].animation_data_create()
bpy.data.cameras['Camera'].animation_data.nla_tracks.new()
bpy.data.cameras['Camera'].animation_data.nla_tracks[0].strips.new('Zoom', 1, bpy.data.actions['Zoom'])
bpy.ops.export_scene.gltf(filepath='out/nla.glb', export_animation_mode='NLA_TRACKS', export_pointer_animation=True, export_lights=True, export_cameras=True)
