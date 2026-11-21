# WP 7.6: animated MultiresModifier levels are clamped to 0..total_levels
# (rna_MultiresModifier_level_range), not to the static 0..255: a fresh Multires modifier has no
# subdivision level, so keys 0@1 and 3@10 on levels, sculpt_levels and render_levels all read 0
# at frame 10. The values are copied into a probe empty.
# golden: startup=factory tier=T1 tol=1e-5 compare=objects
import bpy
bpy.ops.object.modifier_add(type='MULTIRES')
bpy.data.objects['Cube'].keyframe_insert('modifiers["Multires"].levels', frame=1)
bpy.data.objects['Cube'].keyframe_insert('modifiers["Multires"].sculpt_levels', frame=1)
bpy.data.objects['Cube'].keyframe_insert('modifiers["Multires"].render_levels', frame=1)
bpy.data.objects['Cube'].animation_data.action.layers[0].strips[0].channelbags[0].fcurves.find('modifiers["Multires"].levels').keyframe_points.insert(10, 3.0)
bpy.data.objects['Cube'].animation_data.action.layers[0].strips[0].channelbags[0].fcurves.find('modifiers["Multires"].sculpt_levels').keyframe_points.insert(10, 3.0)
bpy.data.objects['Cube'].animation_data.action.layers[0].strips[0].channelbags[0].fcurves.find('modifiers["Multires"].render_levels').keyframe_points.insert(10, 3.0)
bpy.context.scene.frame_set(10)
bpy.context.scene.collection.objects.link(bpy.data.objects.new('l10', None))
bpy.data.objects['l10'].location = (bpy.data.objects['Cube'].modifiers['Multires'].levels, bpy.data.objects['Cube'].modifiers['Multires'].sculpt_levels, bpy.data.objects['Cube'].modifiers['Multires'].render_levels)
