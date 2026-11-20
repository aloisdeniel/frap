# WP 6.5 fix: applying the first ARMATURE modifier of a use_multi_modifier chain. The coordinates
# handed to the multi modifier (vert_coords_prev) never reach the mesh data, so the applied mesh
# has no extra attribute and the remaining multi modifier deforms the applied mesh.
# golden: startup=factory tier=T1 tol=1e-5 compare=evaluated:Cube,mesh:Cube
import bpy
bpy.ops.object.armature_add(location=(0, 0, -1))
bpy.data.objects['Cube'].vertex_groups.new(name='Bone')
bpy.data.objects['Cube'].vertex_groups['Bone'].add([0, 1, 2, 3], 1.0, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups['Bone'].add([4, 5], 0.5, 'REPLACE')
bpy.data.objects['Cube'].modifiers.new('Rig', 'ARMATURE')
bpy.data.objects['Cube'].modifiers['Rig'].object = bpy.data.objects['Armature']
bpy.data.objects['Cube'].modifiers.new('Rig2', 'ARMATURE')
bpy.data.objects['Cube'].modifiers['Rig2'].object = bpy.data.objects['Armature']
bpy.data.objects['Cube'].modifiers['Rig2'].use_multi_modifier = True
bpy.data.objects['Cube'].modifiers['Rig2'].vertex_group = 'Bone'
bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion = (0.9, 0.2, 0.3, 0.0)
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.modifier_apply(modifier='Rig')
bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion = (0.7, 0.0, 0.7, 0.1)
bpy.data.meshes['Cube'].attributes.keys()
