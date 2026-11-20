# WP 6.5 fix: an armature in Edit Mode (arm->edbo) does not deform: the mesh shows its rest
# shape (verify_armature_deform_valid), both through the modifier and a bone child's matrix.
# golden: startup=factory tier=T1 tol=1e-5 compare=objects,evaluated:Cube,evaluated:Cube.001
import bpy
bpy.ops.object.armature_add(location=(0, 0, -1))
bpy.data.objects['Cube'].vertex_groups.new(name='Bone')
bpy.data.objects['Cube'].vertex_groups['Bone'].add([0, 1, 2, 3], 0.75, 'REPLACE')
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.duplicate()
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.data.objects['Armature'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Armature']
bpy.ops.object.parent_set(type='ARMATURE')
bpy.data.objects['Cube.001'].parent = bpy.data.objects['Armature']
bpy.data.objects['Cube.001'].parent_type = 'BONE'
bpy.data.objects['Cube.001'].parent_bone = 'Bone'
bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion = (0.9, 0.2, 0.3, 0.0)
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Armature'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Armature']
bpy.ops.object.mode_set(mode='EDIT')
