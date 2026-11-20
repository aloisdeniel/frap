# WP 6.9: KHR_animation_pointer import of a hand-written file (import_spec.inputs/pointer.gltf):
# material channels into the imported node tree (CUBICSPLINE base colour -> BEZIER keys with
# AUTO handles on the Base Color and Alpha sockets, STEP metallic -> CONSTANT, the factors of
# every extension the importer reads: emissive strength, IOR, transmission, clearcoat, sheen
# (Mix B input), specular (halved), volume thickness (glTF Material Output group),
# attenuation distance (1 / Density) and colour, the alpha cutoff of the MASK clip nodes),
# lights (intensity through the directional conversion, colour, the spot cones with the same
# keys), cameras in a second, unnamed animation (track Anim_1: yfov -> lens, znear / zfar,
# xmag + ymag -> one ortho_scale); the light range and the camera aspect ratio are not
# imported. The first animation is restored, the camera actions stay stashed.
# golden: startup=factory tier=T2 compare=anim:Main,anim:Anim_1,objects
import bpy
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath='pointer.gltf')
bpy.context.scene.frame_set(12)
bpy.data.objects['Lamp'].location = (bpy.data.lights['Lamp'].energy, bpy.data.lights['Spot'].spot_size, bpy.data.lights['Spot'].spot_blend)
bpy.data.objects['Lamp'].scale = bpy.data.lights['Lamp'].color
bpy.data.objects['Sun'].location = (bpy.data.lights['Sun'].energy, bpy.data.cameras['Cam'].lens, bpy.data.cameras['Side'].ortho_scale)
bpy.data.objects['Quad'].location = bpy.data.materials['Mat'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value[0:3]
bpy.data.objects['Quad'].scale = (bpy.data.materials['Mat'].node_tree.nodes['Principled BSDF'].inputs['Alpha'].default_value, bpy.data.materials['Mat'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value, bpy.data.materials['Mat'].node_tree.nodes['Principled BSDF'].inputs['IOR'].default_value)
bpy.data.objects['Cam'].location = (bpy.data.materials['Mat'].node_tree.nodes['Principled BSDF'].inputs['Specular IOR Level'].default_value, bpy.data.materials['Mat'].node_tree.nodes['Principled BSDF'].inputs['Emission Strength'].default_value, bpy.data.materials['Mat'].node_tree.nodes['Principled BSDF'].inputs['Transmission Weight'].default_value)
bpy.ops.object.empty_add(location=(bpy.data.cameras['Cam'].animation_data.nla_tracks[0].mute, bpy.data.cameras['Cam'].animation_data.nla_tracks[0].strips[0].frame_end, bpy.data.lights['Lamp'].animation_data.nla_tracks[0].strips[0].frame_end))
bpy.context.object.name = bpy.data.cameras['Cam'].animation_data.nla_tracks[0].name + '|' + bpy.data.lights['Lamp'].animation_data.action.name + '|' + bpy.data.lights['Lamp'].animation_data.action_slot.identifier + '|' + bpy.data.cameras['Side'].animation_data.nla_tracks[0].strips[0].action_slot.identifier
