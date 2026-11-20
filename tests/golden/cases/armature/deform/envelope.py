# WP 6.5: ARMATURE modifier envelopes. A two-bone chain with edited envelope radii, distances
# and weights (one bone with envelope_weight 0, one without falloff distance) and a
# non-deforming bone; envelopes only (use_vertex_groups off), envelopes as the fallback of
# vertices no group deforms (both on), and use_envelope_multiply scaling group weights by the
# envelope factor. A grid of points around the bones.
# golden: startup=factory tier=T1 tol=1e-5 compare=evaluated:Grid,evaluated:Grid.001,evaluated:Grid.002
import bpy
bpy.ops.object.armature_add(location=(0, 0, -1))
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].edit_bones['Bone'].tail = (0, 0, 1.0)
bpy.data.armatures['Armature'].edit_bones['Bone'].head_radius = 0.3
bpy.data.armatures['Armature'].edit_bones['Bone'].tail_radius = 0.2
bpy.data.armatures['Armature'].edit_bones['Bone'].envelope_distance = 0.6
bpy.data.armatures['Armature'].edit_bones.new('Top')
bpy.data.armatures['Armature'].edit_bones['Top'].head = (0, 0, 1.0)
bpy.data.armatures['Armature'].edit_bones['Top'].tail = (0, 0.2, 2.0)
bpy.data.armatures['Armature'].edit_bones['Top'].parent = bpy.data.armatures['Armature'].edit_bones['Bone']
bpy.data.armatures['Armature'].edit_bones['Top'].tail_radius = 0.4
bpy.data.armatures['Armature'].edit_bones['Top'].envelope_distance = 0.5
bpy.data.armatures['Armature'].edit_bones['Top'].envelope_weight = 2.0
bpy.data.armatures['Armature'].edit_bones.new('Zero')
bpy.data.armatures['Armature'].edit_bones['Zero'].head = (0.5, 0, 0.5)
bpy.data.armatures['Armature'].edit_bones['Zero'].tail = (1.5, 0, 0.5)
bpy.data.armatures['Armature'].edit_bones['Zero'].envelope_weight = 0.0
bpy.data.armatures['Armature'].edit_bones.new('Hard')
bpy.data.armatures['Armature'].edit_bones['Hard'].head = (-0.5, 0, 0.5)
bpy.data.armatures['Armature'].edit_bones['Hard'].tail = (-1.5, 0, 0.5)
bpy.data.armatures['Armature'].edit_bones['Hard'].head_radius = 0.4
bpy.data.armatures['Armature'].edit_bones['Hard'].tail_radius = 0.4
bpy.data.armatures['Armature'].edit_bones['Hard'].envelope_distance = 0.0
bpy.data.armatures['Armature'].edit_bones.new('Off')
bpy.data.armatures['Armature'].edit_bones['Off'].head = (0, 0.5, 0.5)
bpy.data.armatures['Armature'].edit_bones['Off'].tail = (0, 1.5, 0.5)
bpy.data.armatures['Armature'].edit_bones['Off'].use_deform = False
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=9, size=3.0, location=(0, 0, 0), rotation=(1.5707964, 0, 0.2))
bpy.ops.object.duplicate()
bpy.ops.object.duplicate()
bpy.data.objects['Grid'].modifiers.new('Env', 'ARMATURE')
bpy.data.objects['Grid'].modifiers['Env'].object = bpy.data.objects['Armature']
bpy.data.objects['Grid'].modifiers['Env'].use_vertex_groups = False
bpy.data.objects['Grid'].modifiers['Env'].use_bone_envelopes = True
bpy.data.objects['Grid.001'].vertex_groups.new(name='Top')
bpy.data.objects['Grid.001'].vertex_groups['Top'].add([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19], 0.7, 'REPLACE')
bpy.data.objects['Grid.001'].modifiers.new('Both', 'ARMATURE')
bpy.data.objects['Grid.001'].modifiers['Both'].object = bpy.data.objects['Armature']
bpy.data.objects['Grid.001'].modifiers['Both'].use_bone_envelopes = True
bpy.data.armatures['Armature'].bones['Top'].use_envelope_multiply = True
bpy.data.objects['Grid.002'].vertex_groups.new(name='Top')
bpy.data.objects['Grid.002'].vertex_groups.new(name='Bone')
bpy.data.objects['Grid.002'].vertex_groups['Top'].add([30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59], 1.0, 'REPLACE')
bpy.data.objects['Grid.002'].vertex_groups['Bone'].add([40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 60, 61, 62, 63, 64, 65], 0.5, 'REPLACE')
bpy.data.objects['Grid.002'].modifiers.new('Mult', 'ARMATURE')
bpy.data.objects['Grid.002'].modifiers['Mult'].object = bpy.data.objects['Armature']
bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion = (0.95, 0.0, 0.3, 0.1)
bpy.data.objects['Armature'].pose.bones['Top'].rotation_quaternion = (0.8, 0.5, 0.1, -0.2)
bpy.data.objects['Armature'].pose.bones['Top'].scale = (1.0, 1.4, 1.0)
bpy.data.objects['Armature'].pose.bones['Zero'].location = (0, 0, 1.0)
bpy.data.objects['Armature'].pose.bones['Hard'].location = (0.0, 0.5, 0.0)
bpy.data.objects['Armature'].pose.bones['Off'].location = (1.0, 0, 0)
