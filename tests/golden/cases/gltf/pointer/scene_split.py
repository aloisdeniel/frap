# WP 6.9: KHR_animation_pointer export in SCENE mode, split by datablock (the default):
# the material's embedded node tree animation (NT slot: base colour, alpha -> BLEND, roughness,
# emission strength above 1 -> emissiveFactor normalised + emissiveStrength, specular IOR level
# doubled), a spot light (colour, energy -> W x 683, spot_size / 2, the raw spot_blend as
# innerConeAngle), a perspective camera (lens -> yfov, clip_end -> zfar) and an orthographic one
# (ortho_scale -> xmag / ymag); constant channels are dropped; one animation per light and
# camera, one for the materials.
# golden: startup=factory tier=T2 compare=glb:out/scene_split.glb
import bpy
bpy.context.scene.frame_end = 12
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (1.0, 0.0, 0.0, 1.0)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].keyframe_insert('default_value', frame=1)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.0, 0.25, 1.0, 1.0)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].keyframe_insert('default_value', frame=10)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Alpha'].default_value = 1.0
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Alpha'].keyframe_insert('default_value', frame=2)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Alpha'].default_value = 0.4
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Alpha'].keyframe_insert('default_value', frame=8)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.1
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].keyframe_insert('default_value', frame=1)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.9
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].keyframe_insert('default_value', frame=12)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Emission Color'].default_value = (1.0, 0.5, 0.0, 1.0)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Emission Strength'].default_value = 0.0
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Emission Strength'].keyframe_insert('default_value', frame=1)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Emission Strength'].default_value = 4.0
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Emission Strength'].keyframe_insert('default_value', frame=9)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Specular IOR Level'].default_value = 0.2
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Specular IOR Level'].keyframe_insert('default_value', frame=3)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Specular IOR Level'].default_value = 0.8
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Specular IOR Level'].keyframe_insert('default_value', frame=7)
bpy.data.lights['Light'].type = 'SPOT'
bpy.data.lights['Light'].energy = 200.0
bpy.data.lights['Light'].keyframe_insert('energy', frame=1)
bpy.data.lights['Light'].energy = 800.0
bpy.data.lights['Light'].keyframe_insert('energy', frame=12)
bpy.data.lights['Light'].color = (1.0, 1.0, 1.0)
bpy.data.lights['Light'].keyframe_insert('color', frame=4)
bpy.data.lights['Light'].color = (0.2, 0.6, 1.0)
bpy.data.lights['Light'].keyframe_insert('color', frame=8)
bpy.data.lights['Light'].spot_size = 0.6
bpy.data.lights['Light'].spot_blend = 0.1
bpy.data.lights['Light'].keyframe_insert('spot_size', frame=1)
bpy.data.lights['Light'].keyframe_insert('spot_blend', frame=1)
bpy.data.lights['Light'].spot_size = 1.2
bpy.data.lights['Light'].spot_blend = 0.5
bpy.data.lights['Light'].keyframe_insert('spot_size', frame=6)
bpy.data.lights['Light'].keyframe_insert('spot_blend', frame=11)
bpy.data.cameras['Camera'].lens = 35.0
bpy.data.cameras['Camera'].keyframe_insert('lens', frame=1)
bpy.data.cameras['Camera'].lens = 85.0
bpy.data.cameras['Camera'].keyframe_insert('lens', frame=12)
bpy.data.cameras['Camera'].clip_end = 50.0
bpy.data.cameras['Camera'].keyframe_insert('clip_end', frame=2)
bpy.data.cameras['Camera'].clip_end = 300.0
bpy.data.cameras['Camera'].keyframe_insert('clip_end', frame=5)
bpy.ops.object.camera_add(location=(4, -4, 2))
bpy.context.object.data.name = 'Ortho'
bpy.data.cameras['Ortho'].type = 'ORTHO'
bpy.data.cameras['Ortho'].ortho_scale = 4.0
bpy.data.cameras['Ortho'].keyframe_insert('ortho_scale', frame=1)
bpy.data.cameras['Ortho'].ortho_scale = 10.0
bpy.data.cameras['Ortho'].keyframe_insert('ortho_scale', frame=12)
bpy.ops.export_scene.gltf(filepath='out/scene_split.glb', export_animation_mode='SCENE', export_pointer_animation=True, export_lights=True, export_cameras=True)
