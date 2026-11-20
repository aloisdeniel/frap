# WP 6.5 fix: object operators on bone children (parent_type='BONE') read the posed bone, as
# Object.matrix_world does: translate/rotate, transform_apply and parent_clear(KEEP_TRANSFORM).
# golden: startup=factory tier=T1 tol=1e-5 compare=objects
import bpy
bpy.ops.object.armature_add(location=(0, 0, -1), rotation=(0.2, 0.0, 0.4))
bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion = (0.9, 0.2, 0.3, 0.0)
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.data.objects['Camera'].select_set(True)
bpy.data.objects['Light'].select_set(True)
bpy.data.objects['Armature'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Armature']
bpy.ops.object.parent_set(type='BONE')
bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion = (0.7, 0.2, 0.3, 0.5)
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.transform.translate(value=(1.0, 0.0, 0.0))
bpy.ops.transform.rotate(value=0.5, orient_axis='Z')
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Light'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Light']
bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Camera'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Camera']
bpy.ops.object.parent_clear(type='CLEAR_KEEP_TRANSFORM')
