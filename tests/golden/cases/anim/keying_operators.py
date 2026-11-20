# WP 6.4: the 3D Viewport keying operators. anim.keyframe_insert keys location, rotation (by
# rotation mode) and scale of every selected object; keyframe_insert_by_name applies built-in
# keying sets (Location, BUILTIN_KSI_LocRot, Scaling, the delta and visual sets, Available);
# keyframe_delete_by_name and keyframe_delete_v3d remove the keys at the current frame;
# keyframe_clear_v3d removes the F-curves; DEFAULT keying-set deletion and unknown keying sets
# raise Blender's errors; with nothing selected the insert operator is cancelled.
# golden: startup=factory tier=T1 tol=1e-5 compare=anim:CubeAction,anim:LightAction,objects
import bpy
bpy.ops.object.select_all(action='DESELECT')
bpy.ops.anim.keyframe_insert()
bpy.data.objects['Cube'].select_set(True)
bpy.data.objects['Light'].select_set(True)
bpy.data.objects['Light'].rotation_mode = 'QUATERNION'
bpy.ops.anim.keyframe_insert()
bpy.data.actions['LightAction'].layers[0].strips[0].channelbags[0].fcurves.find('rotation_quaternion', index=3)
bpy.context.scene.frame_current = 10
bpy.data.objects['Cube'].location = (1.0, 2.0, 3.0)
bpy.data.objects['Cube'].rotation_euler.z = 0.75
bpy.ops.anim.keyframe_insert_by_name(type='Location')
bpy.ops.anim.keyframe_insert_by_name(type='BUILTIN_KSI_LocRot')
bpy.context.scene.frame_current = 15
bpy.data.objects['Cube'].scale = (2.0, 2.0, 0.5)
bpy.ops.anim.keyframe_insert_by_name(type='Scaling')
bpy.ops.anim.keyframe_insert_by_name(type='BUILTIN_KSI_DeltaScale')
bpy.ops.anim.keyframe_insert_by_name(type='BUILTIN_KSI_VisualLoc')
bpy.context.scene.frame_current = 18
bpy.ops.anim.keyframe_insert_by_name(type='Available')
bpy.ops.anim.keyframe_insert_by_name(type='Nope')
bpy.ops.anim.keyframe_delete()
bpy.context.scene.frame_current = 15
bpy.ops.anim.keyframe_delete_by_name(type='Scaling')
bpy.context.scene.frame_current = 18
bpy.ops.anim.keyframe_delete_v3d()
bpy.data.objects['Light'].select_set(False)
bpy.context.scene.frame_current = 10
bpy.ops.anim.keyframe_delete_v3d()
bpy.data.objects['Cube'].select_set(False)
bpy.data.objects['Light'].select_set(True)
bpy.ops.anim.keyframe_clear_v3d()
bpy.ops.anim.keyframe_clear_v3d()
bpy.data.objects['Camera'].location = (bpy.data.objects['Light'].animation_data.action.users, bpy.context.scene.frame_current, bpy.data.actions['CubeAction'].layers[0].strips[0].channelbags[0].fcurves.find('delta_scale', index=1).keyframe_points[0].co.y)
