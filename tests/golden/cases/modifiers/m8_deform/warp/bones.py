# WP 8.3.12: WARP and UV_WARP from a posed bone (`bone_from`, the armature's world matrix times
# the pose matrix) to an empty, a missing bone name (the object's matrix), a deformed object
# that is itself moved, and a `NONE` texture (intensity 0: nothing moves).
# golden: startup=empty compare=evaluated:A,evaluated:B,evaluated:C,evaluated:D,evaluated:E tier=T2
import bpy
bpy.ops.object.armature_add(location=(0.2, 0, 0))
bpy.ops.object.mode_set(mode='POSE')
bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion = (0.9, 0.3, 0.1, 0.2)
bpy.data.objects['Armature'].pose.bones['Bone'].location = (0.1, 0.2, 0)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.empty_add(location=(0.6, -0.1, 0.4), rotation=(0.2, 0.1, 0.5))
bpy.context.object.name = 'To'
bpy.ops.mesh.primitive_grid_add(x_subdivisions=10, y_subdivisions=10, size=2)
bpy.context.object.name = 'A'
bpy.data.objects['A'].modifiers.new(name='M', type='WARP')
bpy.data.objects['A'].modifiers['M'].object_from = bpy.data.objects['Armature']
bpy.data.objects['A'].modifiers['M'].bone_from = 'Bone'
bpy.data.objects['A'].modifiers['M'].object_to = bpy.data.objects['To']
bpy.data.objects['A'].modifiers['M'].falloff_radius = 2
bpy.ops.mesh.primitive_grid_add(x_subdivisions=10, y_subdivisions=10, size=2)
bpy.context.object.name = 'B'
bpy.data.objects['B'].modifiers.new(name='M', type='WARP')
bpy.data.objects['B'].modifiers['M'].object_from = bpy.data.objects['To']
bpy.data.objects['B'].modifiers['M'].object_to = bpy.data.objects['Armature']
bpy.data.objects['B'].modifiers['M'].bone_to = 'Missing'
bpy.data.objects['B'].modifiers['M'].falloff_type = 'LINEAR'
bpy.data.objects['B'].modifiers['M'].falloff_radius = 1.5
bpy.ops.mesh.primitive_grid_add(x_subdivisions=10, y_subdivisions=10, size=2, location=(0.5, 0.5, 0), rotation=(0, 0, 0.7))
bpy.context.object.name = 'C'
bpy.data.objects['C'].modifiers.new(name='M', type='WARP')
bpy.data.objects['C'].modifiers['M'].object_from = bpy.data.objects['Armature']
bpy.data.objects['C'].modifiers['M'].bone_from = 'Bone'
bpy.data.objects['C'].modifiers['M'].object_to = bpy.data.objects['To']
bpy.data.objects['C'].modifiers['M'].use_volume_preserve = True
bpy.data.objects['C'].modifiers['M'].falloff_type = 'SPHERE'
bpy.data.objects['C'].modifiers['M'].falloff_radius = 2.5
bpy.data.textures.new('T', type='NONE')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4, size=2)
bpy.context.object.name = 'D'
bpy.data.objects['D'].modifiers.new(name='M', type='WARP')
bpy.data.objects['D'].modifiers['M'].object_from = bpy.data.objects['Armature']
bpy.data.objects['D'].modifiers['M'].object_to = bpy.data.objects['To']
bpy.data.objects['D'].modifiers['M'].texture = bpy.data.textures['T']
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=6, size=2)
bpy.context.object.name = 'E'
bpy.data.objects['E'].modifiers.new(name='M', type='UV_WARP')
bpy.data.objects['E'].modifiers['M'].object_from = bpy.data.objects['Armature']
bpy.data.objects['E'].modifiers['M'].bone_from = 'Bone'
bpy.data.objects['E'].modifiers['M'].object_to = bpy.data.objects['To']
bpy.data.objects['E'].modifiers['M'].axis_u = 'Z'
bpy.data.objects['E'].modifiers['M'].center = (0.25, 0.75)
