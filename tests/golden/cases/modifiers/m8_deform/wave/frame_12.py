# WP 8.3.12: WAVE at frame 12: circles, X and Y waves (cyclic or not), a falloff radius,
# a lifetime with damping, a narrow offset wave, an inverted vertex group, a start position
# object, and along the normals (Z and X/Y) on a sphere.
# golden: startup=empty compare=evaluated:Circles,evaluated:X,evaluated:Y,evaluated:Falloff,evaluated:Life,evaluated:Narrow,evaluated:Group,evaluated:Start,evaluated:Normals,evaluated:NormalsXY tier=T2
import bpy
bpy.context.scene.frame_set(12)
bpy.ops.object.empty_add(location=(0.4, 0.3, 0.2))
bpy.context.object.name = 'Center'
bpy.ops.mesh.primitive_grid_add(x_subdivisions=16, y_subdivisions=16, size=4)
bpy.context.object.name = 'Circles'
bpy.data.objects['Circles'].modifiers.new(name='M', type='WAVE')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=16, y_subdivisions=16, size=4)
bpy.context.object.name = 'X'
bpy.data.objects['X'].modifiers.new(name='M', type='WAVE')
bpy.data.objects['X'].modifiers['M'].use_y = False
bpy.ops.mesh.primitive_grid_add(x_subdivisions=16, y_subdivisions=16, size=4)
bpy.context.object.name = 'Y'
bpy.data.objects['Y'].modifiers.new(name='M', type='WAVE')
bpy.data.objects['Y'].modifiers['M'].use_x = False
bpy.data.objects['Y'].modifiers['M'].use_cyclic = False
bpy.ops.mesh.primitive_grid_add(x_subdivisions=16, y_subdivisions=16, size=4)
bpy.context.object.name = 'Falloff'
bpy.data.objects['Falloff'].modifiers.new(name='M', type='WAVE')
bpy.data.objects['Falloff'].modifiers['M'].falloff_radius = 1.2
bpy.data.objects['Falloff'].modifiers['M'].speed = 0.1
bpy.ops.mesh.primitive_grid_add(x_subdivisions=16, y_subdivisions=16, size=4)
bpy.context.object.name = 'Life'
bpy.data.objects['Life'].modifiers.new(name='M', type='WAVE')
bpy.data.objects['Life'].modifiers['M'].lifetime = 8
bpy.data.objects['Life'].modifiers['M'].damping_time = 20
bpy.data.objects['Life'].modifiers['M'].time_offset = -2
bpy.ops.mesh.primitive_grid_add(x_subdivisions=16, y_subdivisions=16, size=4)
bpy.context.object.name = 'Narrow'
bpy.data.objects['Narrow'].modifiers.new(name='M', type='WAVE')
bpy.data.objects['Narrow'].modifiers['M'].width = 0.6
bpy.data.objects['Narrow'].modifiers['M'].narrowness = 3
bpy.data.objects['Narrow'].modifiers['M'].height = 0.3
bpy.data.objects['Narrow'].modifiers['M'].start_position_x = 0.5
bpy.data.objects['Narrow'].modifiers['M'].start_position_y = -0.4
bpy.ops.mesh.primitive_grid_add(x_subdivisions=16, y_subdivisions=16, size=4)
bpy.context.object.name = 'Group'
bpy.data.objects['Group'].vertex_groups.new(name='G')
bpy.data.objects['Group'].vertex_groups['G'].add([60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116, 117, 118, 119], 1.0, 'REPLACE')
bpy.data.objects['Group'].vertex_groups['G'].add([120, 121, 122, 123, 124, 125, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147, 148, 149], 0.3, 'REPLACE')
bpy.data.objects['Group'].modifiers.new(name='M', type='WAVE')
bpy.data.objects['Group'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['Group'].modifiers['M'].invert_vertex_group = True
bpy.ops.mesh.primitive_grid_add(x_subdivisions=16, y_subdivisions=16, size=4)
bpy.context.object.name = 'Start'
bpy.data.objects['Start'].modifiers.new(name='M', type='WAVE')
bpy.data.objects['Start'].modifiers['M'].start_position_object = bpy.data.objects['Center']
bpy.ops.mesh.primitive_uv_sphere_add(segments=16, ring_count=8, radius=1.5)
bpy.context.object.name = 'Normals'
bpy.data.objects['Normals'].modifiers.new(name='M', type='WAVE')
bpy.data.objects['Normals'].modifiers['M'].use_normal = True
bpy.ops.mesh.primitive_uv_sphere_add(segments=16, ring_count=8, radius=1.5)
bpy.context.object.name = 'NormalsXY'
bpy.data.objects['NormalsXY'].modifiers.new(name='M', type='WAVE')
bpy.data.objects['NormalsXY'].modifiers['M'].use_normal = True
bpy.data.objects['NormalsXY'].modifiers['M'].use_normal_x = True
bpy.data.objects['NormalsXY'].modifiers['M'].use_normal_y = True
bpy.data.objects['NormalsXY'].modifiers['M'].use_normal_z = False
bpy.data.objects['NormalsXY'].modifiers['M'].speed = -0.05
