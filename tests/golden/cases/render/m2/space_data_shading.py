# golden: startup=factory compare=objects,context tier=T0
# WP 2.3: the UI members of `bpy.context` inside the VIEW_3D override (ARCHITECTURE §8):
# `space_data` is the Layout 3D View (frap: a stub with `type`, `shading`, `region_3d`), its
# shading is writable and routes `studio_light` like `scene.display.shading`; `preferences` is
# readable and its glTF add-on preferences writable. Reads that fail in frap but not in Blender
# would show up as line errors here.
import bpy
bpy.context.space_data.type
bpy.context.space_data.region_3d.view_matrix
bpy.context.space_data.region_3d.view_perspective
bpy.context.space_data.region_3d.is_perspective
bpy.context.space_data.shading.light = 'MATCAP'
bpy.context.space_data.shading.studio_light = 'rim.sl'
bpy.context.space_data.shading.studio_light = 'toon_light.exr'
bpy.context.space_data.shading.light = 'STUDIO'
bpy.context.space_data.shading.studio_light = 'toon_light.exr'
bpy.context.space_data.shading.studio_light = 'outdoor.sl'
bpy.context.space_data.shading.show_xray = True
bpy.context.space_data.shading.color_type = 'NOPE'
bpy.context.space_data.type = 'VIEW_3D'
bpy.context.preferences.edit.use_duplicate_mesh
bpy.context.preferences.edit.keyframe_new_handle_type
bpy.context.preferences.addons['io_scene_gltf2'].module
bpy.context.preferences.addons['io_scene_gltf2'].preferences.allow_embedded_format
bpy.context.preferences.addons['nope']
bpy.context.preferences.addons['io_scene_gltf2'].preferences.KHR_materials_variants_ui = True
bpy.context.window.scene
