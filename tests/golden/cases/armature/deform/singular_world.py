# WP 6.5 fix: a target whose armature world matrix is singular (scale 0 on X). The ARMATURE
# modifier takes the mesh to armature space with Blender's safe inverse (invert_m4_m4_safe of
# world_to_object), the virtual modifier child likewise.
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
bpy.data.objects['Cube.001'].parent_type = 'ARMATURE'
bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion = (0.9, 0.2, 0.3, 0.0)
bpy.data.objects['Armature'].scale = (0.0, 1.0, 1.0)
