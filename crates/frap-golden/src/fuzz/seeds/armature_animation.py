# frap_read fuzz seed: `frap < armature_animation.py` (from crates/frap-golden/src/fuzz/seeds/) writes armature_animation.frap
bpy.ops.object.armature_add(location=(0, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.armature.extrude_move(TRANSFORM_OT_translate={'value': (0, 0, 1)})
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Cube'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Armature']
bpy.ops.object.parent_set(type='ARMATURE_AUTO')
bpy.context.scene.frame_set(1)
bpy.data.objects['Cube'].keyframe_insert(data_path='location', frame=1)
bpy.data.objects['Cube'].location = (0, 2, 0)
bpy.data.objects['Cube'].keyframe_insert(data_path='location', frame=20)
bpy.ops.wm.save_as_mainfile(filepath='armature_animation.frap')
