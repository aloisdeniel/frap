# WP 8.3.11: anim.channels_bake. Blender bakes the selected F-curves an animation editor lists
# (its poll needs one), frap the F-curves of the factory Graph Editor (headless-extended,
# deviations §2): Blender runs the operator in a Graph Editor (`#@blender` temp_override of the
# Dope Sheet area switched to GRAPH_EDITOR). Covered: the selected objects only (the unselected
# Camera keeps its keys), their object data (the Light's energy), a collapsed selected group
# standing for its F-curves (a deselected F-curve in it is baked), an ungrouped deselected
# F-curve left alone, use_scene_range=False with a custom range and step (keys outside kept, the
# segment end keeping its interpolation), remove_outside_range, interpolation_type LIN / CONST,
# the scene range, Pose Mode (only the selected bones' F-curves, every pose object), a range
# whose end is below its start. Blender's operator does not tag the baked actions for the
# depsgraph, so the frame_set at the end would evaluate the actions as they were before
# baking: Blender tags them first (`#@blender update_tag()`); frap is always fresh (D-04).
# golden: startup=factory tier=T1 tol=1e-5 compare=anim:CubeAction,anim:CameraAction,anim:LightAction,anim:Free,anim:ArmatureAction,objects
import bpy
#@blender bpy.context.window.screen.areas[2].type = 'GRAPH_EDITOR'
bpy.data.objects['Cube'].keyframe_insert('location', frame=1)
bpy.data.objects['Cube'].location = (3.0, 1.0, 2.0)
bpy.data.objects['Cube'].keyframe_insert('location', frame=10)
bpy.data.objects['Cube'].location = (0.0, -1.0, 2.0)
bpy.data.objects['Cube'].keyframe_insert('location', frame=20)
bpy.data.objects['Cube'].keyframe_insert('rotation_euler', index=2, frame=3)
bpy.data.objects['Cube'].rotation_euler.z = 1.5
bpy.data.objects['Cube'].keyframe_insert('rotation_euler', index=2, frame=13)
bpy.data.actions['CubeAction'].layers[0].strips[0].channelbags[0].fcurves.find('location', index=1).select = False
bpy.data.actions['CubeAction'].layers[0].strips[0].channelbags[0].fcurves.new('scale', index=0)
bpy.data.actions['CubeAction'].layers[0].strips[0].channelbags[0].fcurves.find('scale', index=0).keyframe_points.insert(2, 1.0)
bpy.data.actions['CubeAction'].layers[0].strips[0].channelbags[0].fcurves.find('scale', index=0).keyframe_points.insert(9, 3.0)
bpy.data.actions['CubeAction'].layers[0].strips[0].channelbags[0].fcurves.new('scale', index=1)
bpy.data.actions['CubeAction'].layers[0].strips[0].channelbags[0].fcurves.find('scale', index=1).keyframe_points.insert(2, 1.0)
bpy.data.actions['CubeAction'].layers[0].strips[0].channelbags[0].fcurves.find('scale', index=1).keyframe_points.insert(9, 3.0)
bpy.data.actions['CubeAction'].layers[0].strips[0].channelbags[0].fcurves.find('scale', index=1).select = False
bpy.data.actions['CubeAction'].layers[0].strips[0].channelbags[0].fcurves.find('rotation_euler', index=2).keyframe_points[0].interpolation = 'CONSTANT'
bpy.data.objects['Camera'].keyframe_insert('location', index=0, frame=1)
bpy.data.objects['Camera'].location.x = 2.0
bpy.data.objects['Camera'].keyframe_insert('location', index=0, frame=5)
bpy.data.lights['Light'].keyframe_insert('energy', frame=2)
bpy.data.lights['Light'].energy = 100.0
bpy.data.lights['Light'].keyframe_insert('energy', frame=6)
bpy.data.objects['Light'].select_set(True)
bpy.data.objects['Camera'].select_set(False)
#@frap bpy.ops.anim.channels_bake(use_scene_range=False, range=(2, 12), step=2.5)
#@blender with bpy.context.temp_override(area=bpy.context.window.screen.areas[2], region=[r for r in bpy.context.window.screen.areas[2].regions if r.type == 'WINDOW'][0]): bpy.ops.anim.channels_bake(use_scene_range=False, range=(2, 12), step=2.5)
bpy.data.objects['Cube'].select_set(False)
#@frap bpy.ops.anim.channels_bake(use_scene_range=False, range=(4, 5), interpolation_type='LIN', remove_outside_range=True)
#@blender with bpy.context.temp_override(area=bpy.context.window.screen.areas[2], region=[r for r in bpy.context.window.screen.areas[2].regions if r.type == 'WINDOW'][0]): bpy.ops.anim.channels_bake(use_scene_range=False, range=(4, 5), interpolation_type='LIN', remove_outside_range=True)
bpy.data.objects['Light'].select_set(False)
bpy.data.objects['Camera'].select_set(True)
#@frap bpy.ops.anim.channels_bake(use_scene_range=False, range=(3, 1), interpolation_type='CONST')
#@blender with bpy.context.temp_override(area=bpy.context.window.screen.areas[2], region=[r for r in bpy.context.window.screen.areas[2].regions if r.type == 'WINDOW'][0]): bpy.ops.anim.channels_bake(use_scene_range=False, range=(3, 1), interpolation_type='CONST')
bpy.context.scene.frame_start = 3
bpy.context.scene.frame_end = 7
#@frap bpy.ops.anim.channels_bake(step=1.5)
#@blender with bpy.context.temp_override(area=bpy.context.window.screen.areas[2], region=[r for r in bpy.context.window.screen.areas[2].regions if r.type == 'WINDOW'][0]): bpy.ops.anim.channels_bake(step=1.5)
bpy.data.objects['Camera'].select_set(False)
bpy.data.actions.new('Free')
bpy.data.actions['Free'].slots.new(id_type='OBJECT', name='Free')
bpy.data.actions['Free'].layers.new('Layer').strips.new(type='KEYFRAME').channelbag(bpy.data.actions['Free'].slots[0], ensure=True).fcurves.new('location', index=0).keyframe_points.insert(1, 5.0)
bpy.ops.object.armature_add(location=(0.0, 0.0, 0.0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.armature.extrude_move(TRANSFORM_OT_translate={"value": (0.0, 0.0, 1.0)})
bpy.ops.object.mode_set(mode='POSE')
bpy.data.objects['Armature'].pose.bones['Bone'].keyframe_insert('location', index=0, frame=1)
bpy.data.objects['Armature'].pose.bones['Bone.001'].keyframe_insert('location', index=0, frame=1)
bpy.data.objects['Armature'].pose.bones['Bone'].location.x = 1.0
bpy.data.objects['Armature'].pose.bones['Bone.001'].location.x = 1.0
bpy.data.objects['Armature'].pose.bones['Bone'].keyframe_insert('location', index=0, frame=5)
bpy.data.objects['Armature'].pose.bones['Bone.001'].keyframe_insert('location', index=0, frame=5)
bpy.data.objects['Armature'].keyframe_insert('location', index=2, frame=1)
bpy.data.objects['Armature'].location.z = 1.0
bpy.data.objects['Armature'].keyframe_insert('location', index=2, frame=4)
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['Bone.001'].select = True
bpy.data.objects['Armature'].select_set(False)
#@frap bpy.ops.anim.channels_bake(step=2)
#@blender with bpy.context.temp_override(area=bpy.context.window.screen.areas[2], region=[r for r in bpy.context.window.screen.areas[2].regions if r.type == 'WINDOW'][0]): bpy.ops.anim.channels_bake(step=2)
bpy.data.objects['Armature'].pose.bones['Bone'].select = True
bpy.data.objects['Armature'].pose.bones['Bone'].hide = True
#@frap bpy.ops.anim.channels_bake(use_scene_range=False, range=(2, 2))
#@blender with bpy.context.temp_override(area=bpy.context.window.screen.areas[2], region=[r for r in bpy.context.window.screen.areas[2].regions if r.type == 'WINDOW'][0]): bpy.ops.anim.channels_bake(use_scene_range=False, range=(2, 2))
bpy.data.objects['Armature'].pose.bones['Bone'].hide = False
#@blender bpy.data.actions['CubeAction'].update_tag()
#@blender bpy.data.actions['ArmatureAction'].update_tag()
bpy.context.scene.frame_set(4)
bpy.data.objects['Camera'].location = bpy.data.objects['Cube'].location
